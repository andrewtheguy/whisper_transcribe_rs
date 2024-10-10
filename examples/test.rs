fn main(){
    use std::thread::available_parallelism;
    let default_parallelism_approx = available_parallelism().unwrap().get();
    println!("default_parallelism_approx: {}", default_parallelism_approx);
}