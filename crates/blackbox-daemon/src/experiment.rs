pub fn run_experiment(val: i32) -> i32 {
    // Initial stable version
    let multiplier = 5;
    let result = val * multiplier;
    println!("Experiment result: {}", result);
    result
}

pub fn risky_operation(input: Option<i32>) -> i32 {
    // BUG INTRODUCED: This will panic if input is None!
    input.expect("Experiment failed at line 12")
}
