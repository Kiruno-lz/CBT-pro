#!/bin/bash

echo ">>> 初始化 PostgreSQL 数据库..."

# Create user
if PGPASSWORD="${PGPASSWORD:-}" psql -d postgres -c "CREATE USER cbtpro WITH PASSWORD 'cbtpro';" 2>&1 | grep -q "already exists"; then
    echo "用户 cbtpro 已存在"
else
    echo "用户 cbtpro 已创建"
fi

# Create database
if PGPASSWORD="${PGPASSWORD:-}" psql -U cbtpro -d postgres -c "CREATE DATABASE cbtpro OWNER cbtpro;" 2>&1 | grep -q "already exists"; then
    echo "数据库 cbtpro 已存在"
else
    echo "数据库 cbtpro 已创建"
fi

# Grant privileges
if PGPASSWORD="${PGPASSWORD:-}" psql -U cbtpro -d cbtpro -c "GRANT ALL PRIVILEGES ON DATABASE cbtpro TO cbtpro;" 2>/dev/null; then
    echo ""
    echo "✓ 数据库初始化完成"
    echo "连接字符串: postgresql://cbtpro:cbtpro@localhost/cbtpro"
else
    echo "授予权限失败"
    exit 1
fi