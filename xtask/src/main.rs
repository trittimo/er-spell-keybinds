use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command};

#[derive(Deserialize, Serialize)]
struct Config {
    dll_deploy_directory: String,
    ini_deploy_directory: String,
    modengine_start_script_path: String,
}

fn main() {
    // We move the logic to a wrapper so we can handle the final exit code in one place
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("deploy");

    let is_debug = args.iter().any(|arg| arg == "--debug");
    let force = args.iter().any(|arg| arg == "--force");
    let profile = if is_debug { "debug" } else { "release" };

    let xtask_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let project_root = xtask_dir.parent().ok_or("Could not find project root")?;

    let config = get_or_create_config(project_root)?;

    match command {
        "deploy" => {
            deploy(profile, project_root, &config, force)?;
        }
        "run" => {
            deploy(profile, project_root, &config, force)?;
            run(&config, project_root)?;
        }
        _ => {
            eprintln!("Unknown command '{}'", command);
            eprintln!("Available commands: deploy, run");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn deploy(
    profile: &str,
    project_root: &Path,
    config: &Config,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !force {
        validate_paths(config)?;
    }

    build_project(profile)?;

    println!("Deploying artifacts...");
    let target_dir = project_root.join("target").join(profile);

    deploy_file(
        &target_dir.join("eldenring_remapper.dll"),
        &config.dll_deploy_directory,
        force,
    )?;
    deploy_file(
        &project_root.join("eldenring_remapper.ini"),
        &config.ini_deploy_directory,
        force,
    )?;
    let _ = deploy_file(
        &target_dir.join("eldenring_remapper.pdb"),
        &config.dll_deploy_directory,
        force,
    );

    println!("All files deployed successfully!");
    Ok(())
}

fn run(config: &Config, project_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Launching game via script: '{}'",
        &config.modengine_start_script_path
    );

    let script_path = Path::new(&config.modengine_start_script_path);
    let script_path = if script_path.is_relative() {
        project_root.join(script_path)
    } else {
        script_path.to_path_buf()
    };

    if !script_path.exists() {
        return Err(format!(
            "Launch script not found at: '{}'\nPlease check your env.json.",
            config.modengine_start_script_path
        )
        .into());
    }

    let script_dir = script_path.parent().ok_or("Could not determine script directory")?;
    match script_path.extension() {
        Some(ext) if ext == "ps1" => {
            let script_path_absolute = script_path.canonicalize()?.display().to_string();
            Command::new("powershell.exe")
                .args(&["-ExecutionPolicy", "Bypass", "-File", &script_path_absolute.replacen(r"\\?\", "", 1)])
                .current_dir(script_dir)
                .spawn()?;
        },
        _ => {
            let powershell_command = format!("Start-Process -FilePath '{}' -WorkingDirectory '{}'", script_path.display(), script_dir.display());
            Command::new("powershell")
                .args(&["-Command", &powershell_command])
                .spawn()?;
        }
    }

    println!("Game process started, working from directory: {}", script_dir.display());
    Ok(())
}

fn get_or_create_config(root: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let path = root.join("env.json");

    if !path.exists() {
        println!("env.json not found in project root.");
        print!("Would you like to create a default one? (y/n): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            let default = Config {
                dll_deploy_directory: r"C:\ModEngine2\YourInstance\dlls".to_string(),
                ini_deploy_directory: r"C:\ModEngine2\YourInstance\dlls".to_string(),
                modengine_start_script_path: r"C:\ModEngine2\launchmod_eldenring.bat".to_string(),
            };
            fs::write(&path, serde_json::to_string_pretty(&default)?)?;
            println!("Created env.json. Please edit it with your actual paths.\n");
        } else {
            return Err("Aborting: env.json is required for deployment.".into());
        }
    }

    let content = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content).map_err(|e| {
        format!(
            "Failed to parse env.json: {}\nPerhaps the 'modengine_start_script_path' field is missing?",
            e
        )
    })?;
    Ok(config)
}

fn validate_paths(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let targets = [&config.dll_deploy_directory, &config.ini_deploy_directory];
    let mut invalid = Vec::new();

    for path in targets {
        if !Path::new(path).exists() {
            invalid.push(path.to_string());
        }
    }

    if !invalid.is_empty() {
        let mut msg = String::from("DEPLOYMENT ABORTED: Missing directories:\n");
        for path in invalid {
            msg.push_str(&format!("   - {}\n", path));
        }
        msg.push_str("\nUse '--force' to create these folders automatically.");
        return Err(msg.into());
    }
    Ok(())
}

fn build_project(profile: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Building eldenring-remapper in {} mode...", profile);
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--package", "eldenring-remapper"]);
    if profile == "release" {
        cmd.arg("--release");
    }

    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("Build failed with exit code: {:?}", status.code()).into());
    }
    Ok(())
}

fn deploy_file(src: &Path, dest_dir_str: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let dest_dir = Path::new(dest_dir_str);
    if force && !dest_dir.exists() {
        fs::create_dir_all(dest_dir)?;
    }

    let file_name = src.file_name().ok_or("Source path has no filename")?;
    
    if !src.exists() {
        return Err(format!("Source file {:?} not found.", src).into());
    }

    fs::copy(src, dest_dir.join(file_name))?;
    println!("  -> Copied {:?} to {}", file_name, dest_dir_str);
    Ok(())
}