use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Task {
    name: String,
    completed: bool,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Add a new task.
    Add {
        /// Task name.
        name: String,
    },
    /// List all tasks.
    List,
    /// Mark a task as complete by its 1-based ID.
    Complete {
        /// Task ID shown in the list command.
        id: usize,
    },
    DelName {
        name: String,
    },
    DelId {
        id: usize,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tasks.json");

    match cli.command {
        Commands::Add { name } => add_task(&file_path, name)?,
        Commands::List => list_tasks(&file_path)?,
        Commands::Complete { id } => complete_task(&file_path, id)?,
        Commands::DelName { name } => delete_by_name(&file_path, name)?,
        Commands::DelId { id } => delete_by_id(&file_path, id)?,
    }

    Ok(())
}

fn read_tasks(path: &Path) -> io::Result<Vec<Task>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let reader = BufReader::new(file);
    match serde_json::from_reader(reader) {
        Ok(tasks) => Ok(tasks),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {error}", path.display()),
        )),
    }
}

fn write_tasks(path: &Path, tasks: &[Task]) -> io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    let mut writer = BufWriter::new(file);

    serde_json::to_writer_pretty(&mut writer, tasks)?;
    writer.flush()?;
    Ok(())
}

fn add_task(path: &Path, name: String) -> io::Result<()> {
    let mut tasks = read_tasks(path)?;
    let task = Task {
        name,
        completed: false,
    };

    println!("Added task: '{}'", task.name);
    tasks.push(task);
    write_tasks(path, &tasks)
}

fn list_tasks(path: &Path) -> io::Result<()> {
    let tasks = read_tasks(path)?;

    if tasks.is_empty() {
        println!("No tasks yet. Use 'cargo run -p todo_cli -- add \"task\"' to create one.");
        return Ok(());
    }

    println!("--- Todo List ---");
    for (index, task) in tasks.iter().enumerate() {
        let status = if task.completed { "[x]" } else { "[ ]" };
        println!("{} {} {}", index + 1, status, task.name);
    }
    println!("-----------------");
    Ok(())
}

fn complete_task(path: &Path, id: usize) -> io::Result<()> {
    let mut tasks = read_tasks(path)?;

    if id == 0 || id > tasks.len() {
        eprintln!("Invalid task id: {id}");
        return Ok(());
    }

    if let Some(task) = tasks.get_mut(id - 1) {
        if task.completed {
            println!("Task is already complete: '{}'", task.name);
            return Ok(());
        }

        task.completed = true;
        println!("Completed task: '{}'", task.name);
        write_tasks(path, &tasks)?;
    }

    Ok(())
}

fn delete_by_name(path: &Path, name: String) -> io::Result<()> {
    let mut tasks = read_tasks(path)?;
    let Some(index) = tasks.iter().position(|task| task.name == name) else {
        eprintln!("Task not found: '{name}'");
        return Ok(());
    };

    let removed_task = tasks.remove(index);
    println!("Deleted task: '{}'", removed_task.name);
    write_tasks(path, &tasks)
}

fn delete_by_id(path: &Path, id: usize) -> io::Result<()> {
    let mut tasks = read_tasks(path)?;
    if id <= 0 || id > tasks.len() {
        eprintln!("Invalid task id: {id}");
        return Ok(());
    }

    let removed_task = tasks.remove(id - 1);
    println!("Deleted task: '{}'", removed_task.name);
    write_tasks(path, &tasks)
}
