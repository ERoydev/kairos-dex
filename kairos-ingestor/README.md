
## Refined Flow
1. Cursor Manager  → load cursors for [perp, lp]
2. Catch-up Worker → for each program:
     if cursor exists:
         sigs = getSignaturesForAddress(program, until=cursor)   // gap fill
     else:
         apply first-run policy (usually: skip, start from now)
     push sigs to Queue Publisher
3. Stream Subscriber → open WebSocket, subscribe to both programs
4. On each new event → push to Queue Publisher + update cursor


## Mental model
```bash
Catch-up Worker  = "fill the gap"   (runs once at startup)
Stream Subscriber = "live feed"     (runs forever after)
```