#!/bin/bash
# set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && cd .. && pwd)"

export DATABASE_URL="${DATABASE_URL:-postgresql://cbtpro:cbtpro@localhost/cbtpro}"
export REDIS_URL="${REDIS_URL:-redis://localhost:6379/0}"
export RUST_LOG="${RUST_LOG:-info}"
export REST_PORT="${REST_PORT:-8080}"
export WS_PORT="${WS_PORT:-8081}"

echo "=========================================="
echo "   CBT-Pro 本地开发环境启动脚本"
echo "=========================================="
echo ""
echo "配置:"
echo "  DATABASE_URL: $DATABASE_URL"
echo "  REDIS_URL:    $REDIS_URL"
echo "  REST_PORT:    $REST_PORT"
echo "  WS_PORT:      $WS_PORT"
echo ""

check_service() {
    local name=$1
    local port=$2
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo "✓ $name (端口 $port) 已运行"
        return 0
    else
        echo "✗ $name (端口 $port) 未运行"
        return 1
    fi
}

check_postgres() {
    if command -v psql >/dev/null 2>&1; then
        psql -U cbtpro -d cbtpro -c "SELECT 1" >/dev/null 2>&1 && echo "✓ PostgreSQL 数据库连接正常" || echo "✗ PostgreSQL 连接失败"
    else
        echo "⚠ psql 命令不可用，跳过数据库检查"
    fi
}

check_redis() {
    if command -v redis-cli >/dev/null 2>&1; then
        redis-cli ping >/dev/null 2>&1 && echo "✓ Redis 连接正常" || echo "✗ Redis 连接失败"
    else
        echo "⚠ redis-cli 命令不可用，跳过 Redis 检查"
    fi
}

echo ">>> 检查依赖服务..."
MISSING=0

if check_service "PostgreSQL" 5432; then
    check_postgres
else
    echo "  请启动 PostgreSQL: brew services start postgresql@15"
    MISSING=1
fi

if check_service "Redis" 6379; then
    check_redis
else
    echo "  请启动 Redis: brew services start redis"
    MISSING=1
fi

if [ $MISSING -eq 1 ]; then
    echo ""
    echo "请先启动缺失的服务，然后重新运行此脚本。"
    exit 1
fi

wait_for_port() {
    local port=$1
    local timeout=${2:-60}
    local elapsed=0
    while ! lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; do
        sleep 1
        elapsed=$((elapsed + 1))
        if [ $elapsed -ge $timeout ]; then
            echo "✗ 等待端口 $port 超时 (${timeout}s)"
            return 1
        fi
    done
    echo "✓ 端口 $port 已就绪"
    return 0
}

cleanup_port() {
    local port=$1
    local pid=$(lsof -ti :$port 2>/dev/null)
    if [ -n "$pid" ]; then
        echo "  清理占用端口 $port 的进程 (PID: $pid)"
        kill $pid 2>/dev/null || kill -9 $pid 2>/dev/null
        sleep 1
    fi
}

echo ""
echo ">>> 启动 Rust 后端..."
cleanup_port $REST_PORT
cleanup_port $WS_PORT

if [ ! -f "$PROJECT_ROOT/rust_core/Cargo.toml" ]; then
    echo "✗ Rust 项目未找到: $PROJECT_ROOT/rust_core"
    exit 1
fi
cd "$PROJECT_ROOT/rust_core"

echo "  编译 Rust 后端（首次可能需要约 1 分钟）..."
if ! cargo build --release > /tmp/rust_backend.log 2>&1; then
    echo "✗ Rust 后端编译失败"
    echo "错误日志:"
    tail -30 /tmp/rust_backend.log
    exit 1
fi

echo "  运行 Rust 后端..."
./target/release/cbt-pro-api > /tmp/rust_backend.log 2>&1 &
RUST_PID=$!

echo "  等待后端端口就绪..."
if ! wait_for_port $REST_PORT 60; then
    echo "✗ Rust 后端启动失败（REST 端口未就绪）"
    echo "错误日志:"
    tail -30 /tmp/rust_backend.log
    exit 1
fi

if ! wait_for_port $WS_PORT 60; then
    echo "✗ Rust 后端启动失败（WebSocket 端口未就绪）"
    echo "错误日志:"
    tail -30 /tmp/rust_backend.log
    exit 1
fi

echo "  Rust 后端 PID: $RUST_PID"

echo ""
echo ">>> 启动前端开发服务器..."
cleanup_port 3000

if [ ! -f "$PROJECT_ROOT/frontend/package.json" ]; then
    echo "✗ 前端项目未找到: $PROJECT_ROOT/frontend"
    exit 1
fi
if [ ! -d "$PROJECT_ROOT/frontend/node_modules" ] && [ ! -d "$PROJECT_ROOT/frontend/.bun" ]; then
    echo "  安装前端依赖..."
    cd "$PROJECT_ROOT/frontend" && bun install
fi
cd "$PROJECT_ROOT/frontend"
bun run dev > /tmp/frontend.log 2>&1 &
FRONTEND_PID=$!
sleep 3
if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    echo "✗ 前端启动失败"
    echo "错误日志:"
    tail -30 /tmp/frontend.log
    exit 1
fi
echo "  前端 PID: $FRONTEND_PID"

echo ""
echo "=========================================="
echo "   服务已启动！"
echo "=========================================="
echo ""
echo "  前端:    http://localhost:3000"
echo "  REST API: http://localhost:$REST_PORT"
echo "  WebSocket: ws://localhost:$WS_PORT"
echo ""
echo "按 Ctrl+C 停止所有服务"
echo ""

cleanup() {
    echo ""
    echo ">>> 停止服务..."
    kill $RUST_PID 2>/dev/null || true
    kill $FRONTEND_PID 2>/dev/null || true
    echo "已停止所有服务"
    exit 0
}

trap cleanup SIGINT SIGTERM

wait $RUST_PID $FRONTEND_PID