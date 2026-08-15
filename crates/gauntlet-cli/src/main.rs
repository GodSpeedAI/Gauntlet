//! Command surface composition root (spec §4.1, §20.1): the only place
//! adapters are constructed and wired into the application layer. The command
//! surface itself lands in plan Task 20; this stub exists so the workspace
//! shape and version are observable from Task 2 onward.

fn main() {
    println!("gauntlet {}", env!("CARGO_PKG_VERSION"));
    println!("command surface lands in Task 20");
}
