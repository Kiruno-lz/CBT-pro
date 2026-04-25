# CBT-Pro API Specification

## OpenAPI 3.0 Specification

```yaml
openapi: "3.0.3"
info:
  title: CBT-Pro API
  version: "1.0.0"
  description: |
    Institutional-grade cryptocurrency quantitative backtesting system API.

    ## Authentication
    API key authentication is planned for future releases. Currently, the API
    is open for local development. Do not expose the engine to the public
    internet without authentication.

    ## Rate Limiting
    - REST endpoints: 100 requests per minute per IP
    - WebSocket: Max 5 concurrent connections per IP
    - Backtest start: Max 3 concurrent backtests per IP

    ## Error Responses
    All error responses follow the standard Error schema with `code`,
    `message`, and optional `details` fields.

servers:
  - url: http://localhost:8080/api/v1
    description: Local development server

paths:
  /health:
    get:
      summary: Health check endpoint
      description: Returns 200 OK when the engine is ready to accept requests.
      responses:
        '200':
          description: Engine is healthy
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    enum: [ok]
                  version:
                    type: string

  /backtest/start:
    post:
      summary: Start a new backtest
      description: |
        Initiates a backtest run with the given configuration, strategy,
        and time range. Returns immediately with a backtest ID; the engine
        runs the backtest asynchronously.
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [config, strategy_id, timeframe, start_time, end_time]
              properties:
                config:
                  $ref: '#/components/schemas/EngineConfig'
                strategy_id:
                  type: string
                  description: Registered strategy identifier
                timeframe:
                  type: string
                  enum: [1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w]
                  description: Candle timeframe for the backtest
                start_time:
                  type: integer
                  format: int64
                  description: Start timestamp in milliseconds (Unix epoch)
                end_time:
                  type: integer
                  format: int64
                  description: End timestamp in milliseconds (Unix epoch)
      responses:
        '200':
          description: Backtest started successfully
          content:
            application/json:
              schema:
                type: object
                required: [backtest_id, status, total_bars]
                properties:
                  backtest_id:
                    type: string
                    description: Unique backtest run identifier
                  status:
                    type: string
                    enum: [running, queued]
                    description: Current backtest status
                  total_bars:
                    type: integer
                    format: int64
                    description: Total number of bars in the backtest range
        '400':
          description: Invalid request parameters
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
        '409':
          description: Maximum concurrent backtests exceeded
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
        '422':
          description: Unknown strategy ID
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /backtest/{id}/pause:
    post:
      summary: Pause a running backtest
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
          description: Backtest ID
      responses:
        '200':
          description: Backtest paused
          content:
            application/json:
              schema:
                type: object
                required: [status]
                properties:
                  status:
                    type: string
                    enum: [paused]
        '404':
          description: Backtest not found
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
        '409':
          description: Backtest is not in a pausable state
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /backtest/{id}/resume:
    post:
      summary: Resume a paused backtest
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
          description: Backtest ID
      responses:
        '200':
          description: Backtest resumed
          content:
            application/json:
              schema:
                type: object
                required: [status]
                properties:
                  status:
                    type: string
                    enum: [running]
        '404':
          description: Backtest not found
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /backtest/{id}/state:
    get:
      summary: Get current engine state
      description: Returns the latest EngineSnapshot for the backtest.
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
          description: Backtest ID
      responses:
        '200':
          description: Current engine snapshot
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/EngineSnapshot'
        '404':
          description: Backtest not found
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /backtest/{id}/result:
    get:
      summary: Get backtest result (when complete)
      description: |
        Returns the final BacktestResult. This endpoint returns 202
        Accepted if the backtest is still running.
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
          description: Backtest ID
      responses:
        '200':
          description: Backtest complete — final result
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/BacktestResult'
        '202':
          description: Backtest still running
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    enum: [running]
                  progress_pct:
                    type: number
                    description: Percentage of bars processed
        '404':
          description: Backtest not found
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /order:
    post:
      summary: Submit an order
      description: |
        Submit a manual order to the engine. Orders are validated against
        margin requirements and current position state.
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/OrderRequest'
      responses:
        '200':
          description: Order processed
          content:
            application/json:
              schema:
                type: object
                required: [order_id, status]
                properties:
                  order_id:
                    type: string
                  status:
                    type: string
                    enum: [filled, partial, rejected]
                  fill:
                    $ref: '#/components/schemas/OrderFill'
        '400':
          description: Invalid order parameters
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
        '422':
          description: Insufficient margin
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

  /indicators:
    get:
      summary: Get indicator values
      description: |
        Calculate and return technical indicator values for the given
        symbol and timeframe.
      parameters:
        - name: symbol
          in: query
          required: true
          schema:
            type: string
          description: Trading pair symbol (e.g., BTC-USDT)
        - name: timeframe
          in: query
          required: true
          schema:
            type: string
            enum: [1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w]
          description: Candle timeframe
        - name: indicators
          in: query
          required: true
          schema:
            type: string
          description: |
            Comma-separated indicator names. Supported:
            `ema_9`, `ema_21`, `rsi_14`, `macd`, `bollinger_20_2`, `atr_14`, `vwap`
      responses:
        '200':
          description: Indicator values
          content:
            application/json:
              schema:
                type: object
                additionalProperties:
                  oneOf:
                    - type: string
                    - type: number
                    - type: array
                      items:
                        type: object
        '400':
          description: Unknown indicator or invalid parameters
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'

components:
  schemas:
    Error:
      type: object
      required: [code, message]
      properties:
        code:
          type: string
          description: Machine-readable error code
        message:
          type: string
          description: Human-readable error message
        details:
          type: object
          description: Additional error context

    EngineConfig:
      type: object
      required: [symbol, initial_balance, margin_mode, default_leverage]
      properties:
        symbol:
          type: string
          description: Trading pair symbol (e.g., BTC-USDT)
        initial_balance:
          type: string
          description: Initial account balance as decimal string
        margin_mode:
          type: string
          enum: [Isolated, Cross]
          description: Margin mode for positions
        default_leverage:
          type: string
          description: Default leverage as decimal string (e.g., "10")
        maker_fee_rate:
          type: string
          description: Maker fee rate (default: 0.001)
        taker_fee_rate:
          type: string
          description: Taker fee rate (default: 0.005)
        maintenance_margin_rate:
          type: string
          description: Liquidation threshold margin rate (default: 0.005)
        funding_interval_hours:
          type: integer
          description: Funding rate interval in hours (default: 8)
        cost_basis_method:
          type: string
          enum: [FIFO, LIFO, WeightedAverage]
          description: P&L calculation method
        execution_delay_bars:
          type: integer
          description: Bars to delay signal execution (default: 1)
        allow_future_data:
          type: boolean
          description: |
            **DANGER**: Allow strategy to access future bars.
            Must be `false` in production. Only enable for internal testing.
        risk_free_rate:
          type: number
          description: Annual risk-free rate for Sharpe ratio (default: 0.02)

    StandardBar:
      type: object
      required: [timestamp, open, high, low, close, volume, symbol, exchange]
      properties:
        timestamp:
          type: integer
          format: int64
          description: Unix timestamp in milliseconds
        open:
          type: string
          description: Open price as decimal string
        high:
          type: string
          description: High price as decimal string
        low:
          type: string
          description: Low price as decimal string
        close:
          type: string
          description: Close price as decimal string
        volume:
          type: string
          description: Volume as decimal string
        symbol:
          type: string
          description: Trading pair symbol
        exchange:
          type: string
          description: Exchange identifier
        confirmed:
          type: boolean
          description: Whether the bar/k-line is closed

    Position:
      type: object
      required: [id, symbol, direction, status, current_size, average_entry_price]
      properties:
        id:
          type: string
          description: Unique position UUID
        symbol:
          type: string
        direction:
          type: string
          enum: [Long, Short]
        status:
          type: string
          enum: [Open, PartiallyClosed, Closed]
        entries:
          type: array
          items:
            $ref: '#/components/schemas/PositionLeg'
          description: All entry legs for this position
        current_size:
          type: string
          description: Remaining position size
        average_entry_price:
          type: string
          description: Weighted average entry price
        realized_pnl:
          type: string
          description: Realized P&L for closed portions
        unrealized_pnl:
          type: string
          description: Unrealized P&L at mark price
        opened_at:
          type: integer
          format: int64
          description: Position open timestamp
        updated_at:
          type: integer
          format: int64
          description: Last update timestamp

    PositionLeg:
      type: object
      required: [entry_price, quantity, timestamp, order_id]
      properties:
        entry_price:
          type: string
          description: Entry price for this leg
        quantity:
          type: string
          description: Quantity filled for this leg
        timestamp:
          type: integer
          format: int64
          description: Fill timestamp
        order_id:
          type: string
          description: Originating order ID

    OrderRequest:
      type: object
      required: [symbol, side, direction, order_type, quantity]
      properties:
        order_id:
          type: string
          description: Optional client-provided order ID (UUID auto-generated if omitted)
        symbol:
          type: string
        side:
          type: string
          enum: [Buy, Sell]
        direction:
          type: string
          enum: [Long, Short]
        order_type:
          type: string
          enum: [Market, Limit, StopMarket]
        limit_price:
          type: string
          description: Required for Limit orders
        stop_price:
          type: string
          description: Required for StopMarket orders
        quantity:
          type: string
        margin_mode:
          type: string
          enum: [Isolated, Cross]
        leverage:
          type: string
        timestamp:
          type: integer
          format: int64
        strategy_id:
          type: string
          description: Strategy that generated this order
        signal_strength:
          type: number
          description: 0.0 - 1.0 signal confidence
        signal_reason:
          type: string
          description: Human-readable signal reason

    OrderFill:
      type: object
      required: [order_id, symbol, side, direction, filled_price, filled_quantity, fee, timestamp]
      properties:
        order_id:
          type: string
        position_id:
          type: string
          description: Associated position ID (null for rejected orders)
        symbol:
          type: string
        side:
          type: string
          enum: [Buy, Sell]
        direction:
          type: string
          enum: [Long, Short]
        filled_price:
          type: string
          description: Average fill price
        filled_quantity:
          type: string
          description: Filled quantity
        fee:
          type: string
          description: Trading fee paid
        fee_asset:
          type: string
          description: Asset in which fee was paid
        timestamp:
          type: integer
          format: int64
          description: Fill timestamp
        realized_pnl:
          type: string
          description: Realized P&L for this fill (for reducing/closing orders)

    EngineSnapshot:
      type: object
      required: [timestamp, current_bar, equity, total_trades]
      properties:
        timestamp:
          type: integer
          format: int64
          description: Snapshot timestamp
        current_bar:
          $ref: '#/components/schemas/StandardBar'
        equity:
          type: string
          description: Total account equity
        available_balance:
          type: string
          description: Free margin available
        margin_used:
          type: string
          description: Margin used by open positions
        margin_ratio:
          type: string
          description: margin_used / equity
        unrealized_pnl:
          type: string
          description: Unrealized P&L of open positions
        realized_pnl_today:
          type: string
          description: Realized P&L since last reset
        positions:
          type: array
          items:
            $ref: '#/components/schemas/Position'
          description: Currently open positions
        orders_history:
          type: array
          items:
            $ref: '#/components/schemas/OrderFill'
          description: Completed order fills
        daily_pnl:
          type: array
          items:
            type: array
            items:
              oneOf:
                - type: integer
                  format: int64
                  description: Day timestamp
                - type: string
                  description: Daily P&L value
        max_drawdown:
          type: string
          description: Absolute max drawdown value
        max_drawdown_pct:
          type: string
          description: Max drawdown as percentage
        sharpe_ratio:
          type: number
          description: Annualized Sharpe ratio
        total_trades:
          type: integer
          format: int64
          description: Total number of trades
        winning_trades:
          type: integer
          format: int64
        losing_trades:
          type: integer
          format: int64
        win_rate:
          type: number
          description: Winning trades percentage

    BacktestResult:
      type: object
      required: [backtest_id, final_equity, total_return_pct, total_trades]
      properties:
        backtest_id:
          type: string
        final_equity:
          type: string
        total_return_pct:
          type: number
          description: Total return as percentage
        max_drawdown_pct:
          type: number
          description: Maximum drawdown percentage
        sharpe_ratio:
          type: number
        total_trades:
          type: integer
          format: int64
        win_rate:
          type: number
          description: Win rate percentage
        profit_factor:
          type: number
          description: Gross profit / gross loss
        avg_trade_return:
          type: number
          description: Average return per trade
        daily_pnls:
          type: array
          items:
            type: string
            description: Daily P&L values
        trades:
          type: array
          items:
            $ref: '#/components/schemas/OrderFill'
          description: Complete trade history
```

## WebSocket Events

### Connection

```
ws://localhost:8081/ws
```

The WebSocket connection is stateful. After connecting, the client must
subscribe to channels to receive data.

### Client -> Server Messages

#### Subscribe to backtest state

```json
{
  "type": "subscribe",
  "channel": "backtest_state",
  "backtest_id": "bt_7f3a..."
}
```

Response: `{ "type": "subscribed", "channel": "backtest_state" }`

#### Control playback

```json
{
  "type": "control",
  "action": "play",
  "backtest_id": "bt_7f3a..."
}
```

Actions: `play`, `pause`, `step_forward`, `step_backward`

#### Set playback speed

```json
{
  "type": "control",
  "action": "set_speed",
  "speed": 5.0
}
```

Speed multiplier: `0.1` (10x slow) to `10.0` (10x fast). Default `1.0`.

### Server -> Client Messages

#### Engine snapshot (sent every bar or on state change)

```json
{
  "type": "snapshot",
  "data": {
    "timestamp": 1704067200000,
    "current_bar": {
      "timestamp": 1704067200000,
      "open": "42000.00",
      "high": "42500.00",
      "low": "41800.00",
      "close": "42350.00",
      "volume": "150.5",
      "symbol": "BTC-USDT",
      "exchange": "binance",
      "confirmed": true
    },
    "equity": "100050.50",
    "available_balance": "90000.00",
    "margin_used": "10050.50",
    "margin_ratio": "0.1005",
    "unrealized_pnl": "50.50",
    "realized_pnl_today": "0.00",
    "positions": [],
    "orders_history": [],
    "daily_pnl": [],
    "max_drawdown": "0.00",
    "max_drawdown_pct": "0.00",
    "sharpe_ratio": null,
    "total_trades": 0,
    "winning_trades": 0,
    "losing_trades": 0,
    "win_rate": 0.0
  }
}
```

#### Bar update

```json
{
  "type": "bar_update",
  "bar": {
    "timestamp": 1704070800000,
    "open": "42350.00",
    "high": "42800.00",
    "low": "42200.00",
    "close": "42600.00",
    "volume": "200.0",
    "symbol": "BTC-USDT",
    "exchange": "binance",
    "confirmed": true
  }
}
```

#### Trade fill notification

```json
{
  "type": "trade",
  "fill": {
    "order_id": "ord_abc123",
    "position_id": "pos_def456",
    "symbol": "BTC-USDT",
    "side": "Buy",
    "direction": "Long",
    "filled_price": "42350.00",
    "filled_quantity": "0.1000",
    "fee": "0.42",
    "fee_asset": "USDT",
    "timestamp": 1704067200000,
    "realized_pnl": "0.00"
  }
}
```

#### Signal notification

```json
{
  "type": "signal",
  "signal": {
    "action": "open_long",
    "symbol": "BTC-USDT",
    "quantity": "0.1000",
    "strength": 0.85,
    "reason": "EMA9 crossed above EMA21",
    "timestamp": 1704067200000
  }
}
```

#### Backtest complete

```json
{
  "type": "complete",
  "result": {
    "backtest_id": "bt_7f3a...",
    "final_equity": "145230.50",
    "total_return_pct": 45.23,
    "max_drawdown_pct": 12.5,
    "sharpe_ratio": 1.85,
    "total_trades": 156,
    "win_rate": 58.3,
    "profit_factor": 2.1,
    "avg_trade_return": 1.2,
    "daily_pnls": ["100.50", "-50.25", "200.00"],
    "trades": []
  }
}
```

#### Error

```json
{
  "type": "error",
  "message": "Backtest bt_7f3a... not found"
}
```

## Rate Limiting

| Resource | Limit | Window |
|----------|-------|--------|
| REST API requests | 100 | per minute per IP |
| WebSocket connections | 5 | concurrent per IP |
| Backtest starts | 3 | concurrent per IP |
| Indicator requests | 60 | per minute per IP |

When rate limited, the API returns:

```json
{
  "code": "RATE_LIMITED",
  "message": "Rate limit exceeded. Retry after 60 seconds."
}
```

## Authentication (Future)

API key authentication will be implemented in a future release:

```
Authorization: Bearer <api_key>
```

For the current development phase, no authentication is required.
**Do not expose the engine to the public internet.**
