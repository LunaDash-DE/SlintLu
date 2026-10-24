//! Prints where icons resolve to: `cargo run --example icon_lookup -- firefox kitty 64`.

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let size = args.last().and_then(|s| s.parse().ok()).inspect(|_| {
        args.pop();
    });
    let lookup = slintlu::icons::IconLookup::new(None);
    println!("theme: {}", slintlu::icons::default_theme_name());
    for name in args {
        println!("{name}: {:?}", lookup.lookup(&name, size.unwrap_or(48)));
    }
}
