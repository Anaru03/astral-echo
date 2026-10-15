pub(crate) fn run() {
    let mut arguments = std::env::args().skip(1);
    let result = match arguments.next().as_deref() {
        None => super::run_scenario(),
        Some("--runtime") if arguments.next().is_none() => super::runtime_mode::run(),
        Some("--visual") if arguments.next().is_none() => super::visual::run(),
        Some(argument) => Err(format!("argumento no reconocido: {argument}")),
    };
    if let Err(error) = result {
        eprintln!("ESCENARIO FALLIDO: {error}");
        std::process::exit(1);
    }
}