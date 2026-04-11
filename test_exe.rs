fn main() {
    let current_exe = std::env::current_exe().unwrap();
    let file_name = current_exe.file_name().unwrap().to_str().unwrap().to_string();
    println!("{}", file_name);
}
