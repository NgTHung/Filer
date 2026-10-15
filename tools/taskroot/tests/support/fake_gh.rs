use std::{env, fs, io::{self, Read}, path::PathBuf, process};

fn main() {
    let root = PathBuf::from(env::var_os("FAKE_GH_DIR").unwrap());
    let index = fs::read_to_string(root.join("index")).unwrap_or_else(|_| "0".into());
    let index: usize = index.parse().unwrap();
    let mut request = String::new();
    io::stdin().read_to_string(&mut request).unwrap();
    fs::write(root.join(format!("request-{index}.json")), request).unwrap();
    fs::write(root.join("index"), (index + 1).to_string()).unwrap();
    match fs::read_to_string(root.join(format!("response-{index}.json"))) {
        Ok(response) => print!("{response}"),
        Err(error) => {
            eprintln!("Unexpected GitHub request: {error}");
            process::exit(1);
        }
    }
}
