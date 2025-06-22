use algorist::io::{test_cases, wln};

fn main() {
    test_cases(&mut |scan, w| {
        let (a, b) = scan.u2();
        wln!(w, "{}", a + b);
    });
}
