//! `luna-clock` — zegar i kalendarz środowiska Luna.

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    luna_clock::run_clock()
}
