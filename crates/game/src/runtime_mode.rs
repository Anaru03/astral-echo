use super::*;

pub(crate) fn run() -> Result<(), String> {
    let mut runtime = create_runtime_session()?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout();
    write_runtime_help(&mut output)?;
    loop {
        write!(output, "astral> ").map_err(|error| format!("escribir prompt: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("vaciar prompt: {error}"))?;
        let mut line = String::new();
        let read = input
            .read_line(&mut line)
            .map_err(|error| format!("leer entrada: {error}"))?;
        if read == 0 {
            writeln!(output).map_err(|error| format!("escribir fin de entrada: {error}"))?;
            return Ok(());
        }
        match parse_runtime_command(line.trim_end()) {
            Ok(command) => {
                if !execute_runtime_command(&mut runtime, &command, &mut output)? {
                    return Ok(());
                }
            }
            Err(error) => {
                writeln!(output, "error: {error}")
                    .map_err(|write_error| format!("escribir error de entrada: {write_error}"))?;
            }
        }
    }
}