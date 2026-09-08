
pub struct FLoop {
    interval_timer: u16,
}

/*
The loop will scan all markets, filter out the ones that aren't due (now - last_time >= interval_seconds) 
and push only the ones that are due onto the queue.

*/