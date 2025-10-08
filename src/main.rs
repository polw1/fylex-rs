use anyhow::{Context, Result};
use ncurses::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Utc;

const ROOT: &str = "/home/pdc/dev";
const CONFIG_NAME: &str = "fylex.config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectConfig {
    name: String,
    description: String,
    tags: Vec<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct Project {
    path: PathBuf,
    cfg: Option<ProjectConfig>,
    // Git status independent from config presence
    // 1 = clean (V), 2 = modified (M)
    git_state: Option<u8>,
}

#[derive(Default)]
struct AppState {
    projects: Vec<Project>,
    filtered: Vec<usize>,
    selected: usize,
    filter_text: String,
}

fn scan_projects(root: &str) -> Result<Vec<Project>> {
    let mut v = Vec::new();

    for entry_res in fs::read_dir(root).with_context(|| format!("Reading directory {}", root))? {
        let entry = entry_res?;
        let ty = entry.file_type()?;
        if !ty.is_dir() {
            continue;
        }
        let path: PathBuf = entry.path();
        let cfg = read_config(&path).ok().flatten();
        let git_state = git_status_color(&path).map(|c| c as u8);
        v.push(Project { path, cfg, git_state });
    }

    v.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    Ok(v)
}

fn read_config(dir: &Path) -> Result<Option<ProjectConfig>> {
    let p = dir.join(CONFIG_NAME);
    if !p.exists() {
        return Ok(None);
    }
    let mut s = String::new();
    fs::File::open(&p)?.read_to_string(&mut s)?;
    let cfg: ProjectConfig = serde_json::from_str(&s)?;
    Ok(Some(cfg))
}

fn git_status_color(path: &Path) -> Option<i32> {
    if !path.join(".git").exists() {
        return None;
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;
    if output.stdout.is_empty() {
        Some(1) // green for clean
    } else { // oragen for modification
        Some(2)
    }
}

fn draw(state: &AppState) {
    erase();

    let mut rows = 0;
    let mut cols = 0;
    getmaxyx(stdscr(), &mut rows, &mut cols);

    attron(COLOR_PAIR(1));
    mvhline(0, 0, ' ' as u32, cols);
    let _ = mvprintw(
        0,
        1,
        &format!(
            " Project Manager - root: {ROOT} | ENTER=open in terminal O=open folder Q=quit A=add cfg T=edit tag N=new R=reload type to filter "
        ),
    );
    attroff(COLOR_PAIR(1));

    // ---------- LINHA DE FILTRO ----------
    attron(COLOR_PAIR(3));
    let _ = mvprintw(1, 1, "Filter: ");
    attroff(COLOR_PAIR(3));
    let _ = mvprintw(1, 9, &state.filter_text);

    // ---------- LAYOUT DAS DUAS ÁREAS ----------
    let list_top = 3;
    let list_width = (cols as f32 * 0.40) as i32;
    let detail_left = list_width + 2;

    // ---------- LIST TITLE ----------
    attron(A_BOLD);
    let _ = mvprintw(2, 1, "Projects");
    attroff(A_BOLD);

    // ---------- DRAW PROJECT LINES ----------
    let visible_rows = &state.filtered;
    for (i, &idx) in visible_rows.iter().enumerate() {
        if let Some(p) = state.projects.get(idx) {
            let line = list_top + i as i32;
            if line >= rows - 1 {
                break;
            }

            if i == state.selected {
                attron(COLOR_PAIR(2));
                mvhline(line, 1, ' ' as u32, list_width - 2);
                let label =
                    p.cfg.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| {
                        p.path.file_name().unwrap().to_string_lossy().to_string()
                    });
                let _ = mvprintw(line, 2, &label);
                attroff(COLOR_PAIR(2));
                let status = p
                    .git_state;
                if let Some(status) = status {
                    match status {
                        1 => {
                            attron(COLOR_PAIR(4));
                            let _ = mvprintw(line, 2 + label.len() as i32, " | V");
                            attroff(COLOR_PAIR(4));
                        },
                        2 => {
                            attron(COLOR_PAIR(4));
                            let _ = mvprintw(line, 2 + label.len() as i32, " | M");
                            attroff(COLOR_PAIR(5));
                        }
                        _ => {}
                    }
                }
            } else {
                let label =
                    p.cfg.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| {
                        p.path.file_name().unwrap().to_string_lossy().to_string()
                    });
                let _ = mvprintw(line, 2, &label);
                let status = p
                    .git_state;
                if let Some(status) = status {
                    match status {
                        1 => {
                            // Use initialized green pair for clean git state
                            attron(COLOR_PAIR(4));
                            let _ = mvprintw(line, 2 + label.len() as i32, " | V");
                            attroff(COLOR_PAIR(4));
                        },
                        2 => {
                            attron(COLOR_PAIR(4));
                            let _ = mvprintw(line, 2 + label.len() as i32, " | M");
                            attroff(COLOR_PAIR(4));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // ---------- DETAIL AREA ----------
    attron(A_BOLD);
    let _ = mvprintw(2, detail_left, "Details");
    attroff(A_BOLD);

    if let Some(p) = current_project(state) {
        let mut y = 3;

        // -- Name --
        let label_name = p
            .cfg
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("(No config file set)");
        attron(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left, "Name: ");
        attroff(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left + 6, label_name);
        y += 1;

        // -- Path --
        attron(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left, "Path: ");
        attroff(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left + 6, &p.path.to_string_lossy());
        y += 1;

        // -- Tags --
        let tags_str = p
            .cfg
            .as_ref()
            .map(|c| c.tags.join(", "))
            .unwrap_or_default();
        attron(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left, "Tags: ");
        attroff(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left + 6, &tags_str);
        y += 1;

        // -- Description --
        let desc = p
            .cfg
            .as_ref()
            .map(|c| c.description.clone())
            .unwrap_or_default();
        attron(COLOR_PAIR(3));
        let _ = mvprintw(y, detail_left, "Description: ");
        attroff(COLOR_PAIR(3));

        wrap_print(
            y,
            detail_left + 6,
            desc.as_str(),
            (cols - detail_left - 2) as usize,
            rows - y - 1,
        );
    }

    refresh();
}

fn wrap_print(mut y: i32, x: i32, text: &str, width: usize, max_lines: i32) {
    let mut line = String::new();
    let mut used = 0;

    for word in text.split_whitespace() {
        if line.len() + 1 + word.len() > width && !line.is_empty() {
            if used >= max_lines {
                break;
            }
            let _ = mvprintw(y, x, &line);
            y += 1;
            used += 1;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() && used < max_lines {
        let _ = mvprintw(y, x, &line);
    }
}

fn current_project(state: &AppState) -> Option<&Project> {
    state
        .filtered
        .get(state.selected)
        .and_then(|&i| state.projects.get(i))
}

fn rebuild_filter(state: &mut AppState) {
    let f = state.filter_text.to_lowercase();
    state.filtered.clear();

    // if f.is_empty() {
    //     state.selected = 0;
    //     return;
    // }

    for (i, p) in state.projects.iter().enumerate() {
        let name = p
            .cfg
            .as_ref()
            .map(|c| c.name.to_lowercase())
            .unwrap_or_else(|| p.path.file_name().unwrap().to_string_lossy().to_lowercase());

        let tags = p
            .cfg
            .as_ref()
            .map(|c| {
                c.tags
                    .iter()
                    .map(|t| t.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let hay = format!("{name} {tags}");
        if hay.contains(&f) {
            state.filtered.push(i);
        }
    }
    if state.selected >= state.filtered.len() {
        state.selected = state.filtered.len().saturating_sub(1);
    }
}

fn open_in_terminal(path: &Path) -> Result<()> {
    // Restore terminal before handing control to the user's shell
    endwin();

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Clear screen before handing off to the shell (exec never returns on success)
        print!("\x1B[2J\x1B[H");
        let _ = std::io::stdout().flush();
        let err = Command::new(&shell).current_dir(path).exec();
        // If exec returns, it failed
        Err(anyhow::anyhow!(format!("exec failed: {err}")))
    }

    #[cfg(not(unix))]
    {
        // Fallback: run and wait, then exit with the same code
        let status = Command::new(&shell).current_dir(path).status()?;
        process::exit(status.code().unwrap_or(0));
    }
}

fn flash_line(msg: &str, color_pair: i16) {
    let mut rows = 0;
    let mut cols = 0;
    getmaxyx(stdscr(), &mut rows, &mut cols);

    attron(COLOR_PAIR(color_pair));
    mvhline(rows - 1, 0, ' ' as u32, cols);
    let _ = mvprintw(rows - 1, 1, msg);
    attroff(COLOR_PAIR(color_pair));
    refresh();
    napms(1500);
}

fn flash_error(msg: &str) {
    flash_line(msg, 5);
}

fn flash_ok(msg: &str) {
    flash_line(msg, 4);
}

fn prompt_input(label: &str, initial: &str) -> String {
    let mut rows = 0;
    let mut cols = 0;
    getmaxyx(stdscr(), &mut rows, &mut cols);

    let mut buf = initial.to_string();
    loop {
        attron(COLOR_PAIR(3));
        mvhline(rows - 1, 0, ' ' as u32, cols);
        let _ = mvprintw(rows - 1, 1, label);
        attroff(COLOR_PAIR(3));
        let _ = mvprintw(rows - 1, (label.len() + 1) as i32, &buf);
        mv(rows - 1, (label.len() + 2 + buf.len()) as i32);
        refresh();
        let ch = getch();
        match ch {
            10 => break,
            27 => { buf.clear(); break; }, // ESC to cancel
            127 | KEY_BACKSPACE => { buf.pop(); },
            c if (32..=126).contains(&c) => buf.push(c as u8 as char),
            _ => {}

        }
    }
    buf

}

fn create_new_project(root: &str, name: &str) -> Result<()> {
    let dir = Path::new(root).join(name);
    if dir.exists() {
        return Err(anyhow::anyhow!("Directory already exists"));
    }
    let _ = fs::create_dir_all(&dir);
    // Adicionar git init
    Command::new("git")
        .arg("init")
        .arg(&dir)
        .output()
        .with_context(|| "Failed to initialize git repository")?;
    write_default_config(&dir)
}

fn write_default_config(dir: &Path) -> Result<()> {
    let name = Path::new(dir)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let cfg = ProjectConfig {
        name,
        description: String::new(), 
        tags: Vec::new(),
        created_at: Utc::now().to_rfc3339(),
    };

    write_config(dir, &cfg)
}

fn write_config(dir: &Path, cfg: &ProjectConfig) -> Result<()> {
    let p = dir.join(CONFIG_NAME);
    let s = serde_json::to_string_pretty(cfg)?;
    fs::write(p, s)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut state = AppState::default();
    state.projects = scan_projects(ROOT)?;
    rebuild_filter(&mut state);

    // ncurses init
    initscr();
    raw();
    keypad(stdscr(), true);
    noecho();
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

    if has_colors() {
        start_color();
        // Allow terminal default background/foreground if supported
        let _ = use_default_colors();
        init_pair(1, COLOR_WHITE, COLOR_BLUE); // header
        init_pair(2, COLOR_YELLOW, COLOR_BLACK); // selected
        init_pair(3, COLOR_CYAN, COLOR_BLACK); // labels
        init_pair(4, COLOR_GREEN, COLOR_BLACK); // ok
        init_pair(5, COLOR_RED, COLOR_BLACK); // warn
    }

    loop {
        draw(&state);

        let ch = getch();
        match ch {
            81 => break,
            // Allow filter text input
            // Backspace
            127 | KEY_BACKSPACE => {
                state.filter_text.pop();
                rebuild_filter(&mut state);
            }
            KEY_UP => {
                if state.selected > 0 {
                    state.selected -= 1;
                }
            }
            KEY_DOWN => {
                if state.selected + 1 < state.filtered.len() {
                    state.selected += 1;
                }
            }
            // Enter walk into project folder through terminal
            10 | KEY_ENTER => {
                if let Some(p) = current_project(&state) {
                    match open_in_terminal(&p.path) {
                        Ok(_) => {
                            break;
                        }
                        Err(e) => flash_error(&format!("Terminal open failed: {e}")),
                    }
                }
            }
            // N for create new project folder
            78 => {
                let name = prompt_input("New project name: ","");
                if name.trim().is_empty() {
                    flash_error("Name cannot be empty");
                } else {
                    match create_new_project(ROOT, name.trim()) {
                        Ok(_) => {
                            flash_ok("Project created");
                            state.projects = scan_projects(ROOT)?;
                            rebuild_filter(&mut state);
                        }
                        _ => flash_error("Failed to create project"),
                    }
                }
            }
            c if (32..=126).contains(&c) => {
                state.filter_text.push(c as u8 as char);
                rebuild_filter(&mut state);
            }
            _ => {}
        }
    }

    endwin();
    Ok(())
}
