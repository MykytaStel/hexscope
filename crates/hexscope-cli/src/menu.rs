//! `hexscope menu`: "Clean with hexscope" where files are picked — a Quick
//! Action in Finder, an entry in Windows' Send to, a script in GNOME Files.
//! It runs this program, by its full path, as `clean --verify` on what was
//! chosen, and shows what it said. `--remove` takes it away again.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const NAME: &str = "Clean with hexscope";

/// Where the entry goes on this system, what it is made of, and how to use it.
struct Entry {
    /// The file or bundle the entry is.
    path: PathBuf,
    /// Files to write, relative to `path` (empty for `path` itself).
    files: Vec<(&'static str, String)>,
    /// Runs as a program (the GNOME script); Windows has no such bit.
    #[cfg_attr(not(unix), allow(dead_code))]
    executable: bool,
    how: &'static str,
}

pub(crate) fn run(args: &[String]) -> Result<ExitCode, String> {
    let remove = match args {
        [] => false,
        [flag] if flag == "--remove" => true,
        [other, ..] => return Err(format!("menu takes only --remove, not {other}")),
    };
    let exe = program()?;
    let entry = entry(std::env::consts::OS, &exe)?;
    if remove {
        let gone = if entry.files.iter().any(|(rel, _)| !rel.is_empty()) {
            std::fs::remove_dir_all(&entry.path)
        } else {
            std::fs::remove_file(&entry.path)
        };
        match gone {
            Ok(()) => println!("Removed \"{NAME}\" ({}).", entry.path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("\"{NAME}\" was not there ({}).", entry.path.display())
            }
            Err(e) => return Err(format!("{}: {e}", entry.path.display())),
        }
        return Ok(ExitCode::SUCCESS);
    }
    for (rel, content) in &entry.files {
        let file = if rel.is_empty() {
            entry.path.clone()
        } else {
            entry.path.join(rel)
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&file, content).map_err(|e| format!("{}: {e}", file.display()))?;
        #[cfg(unix)]
        if entry.executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    if std::env::consts::OS == "macos" {
        // Finder lists a new Quick Action once the services are read again.
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs")
            .arg("-update")
            .status();
    }
    println!(
        "Added \"{NAME}\": {}. Each clean copy is saved beside its file as NAME-clean.EXT, \
         and what was removed is shown.\nIt runs {}. Take it away with: hexscope menu --remove",
        entry.how,
        exe.display()
    );
    Ok(ExitCode::SUCCESS)
}

/// This program's path, as it stays after an upgrade: Homebrew's versioned
/// folder goes away with the version, its `bin` link does not.
fn program() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot tell where hexscope is: {e}"))?;
    Ok(stable(&exe).unwrap_or(exe))
}

fn stable(exe: &Path) -> Option<PathBuf> {
    let text = exe.to_str()?;
    let (prefix, _) = text.split_once("/Cellar/hexscope/")?;
    let linked = Path::new(prefix).join("bin/hexscope");
    linked.exists().then_some(linked)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn entry(os: &str, exe: &Path) -> Result<Entry, String> {
    let exe_text = exe.to_string_lossy();
    match os {
        "macos" => Ok(Entry {
            path: home()?.join(format!("Library/Services/{NAME}.workflow")),
            files: vec![
                ("Contents/Info.plist", info_plist()),
                (
                    "Contents/document.wflow",
                    workflow(&finder_script(&exe_text)),
                ),
            ],
            executable: false,
            how: "in Finder, select files, then right-click › Quick Actions",
        }),
        "windows" => {
            let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
            Ok(Entry {
                path: PathBuf::from(appdata).join(format!("Microsoft/Windows/SendTo/{NAME}.cmd")),
                files: vec![("", send_to_script(&exe_text))],
                executable: false,
                how: "in File Explorer, select files, then right-click › Send to",
            })
        }
        "linux" => {
            let data = match std::env::var_os("XDG_DATA_HOME") {
                Some(dir) if !dir.is_empty() => PathBuf::from(dir),
                _ => home()?.join(".local/share"),
            };
            Ok(Entry {
                path: data.join(format!("nautilus/scripts/{NAME}")),
                files: vec![("", files_script(&exe_text))],
                executable: true,
                how: "in Files (GNOME), select files, then right-click › Scripts",
            })
        }
        other => Err(format!(
            "menu works on macOS, Windows and Linux, not {other}"
        )),
    }
}

/// `s` inside single quotes, for sh and zsh.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// What the Quick Action runs: the copies, then what was said, in a dialog,
/// with the folder of the first file left out of the paths.
fn finder_script(exe: &str) -> String {
    format!(
        r#"# Written by `hexscope menu`: a clean copy of each file chosen, beside it.
out=$({} clean --verify "$@" 2>&1)
dir="${{1:h}}/"
out=${{out//${{(b)dir}}/}}
out=$(print -r -- "$out" | head -n 40)
osascript - "$out" <<'END'
on run argv
	display dialog (item 1 of argv) with title "hexscope" buttons {{"OK"}} default button 1
end run
END
"#,
        sh_quote(exe)
    )
}

/// What Send to runs: the copies in a console window that stays open.
fn send_to_script(exe: &str) -> String {
    format!(
        "@echo off\r\nrem Written by \"hexscope menu\": a clean copy of each file sent here, beside it.\r\n\"{}\" clean --verify %*\r\necho.\r\npause\r\n",
        exe.replace('%', "%%")
    )
}

/// What GNOME Files runs: the copies, then what was said in a window, or
/// its last line as a notification.
fn files_script(exe: &str) -> String {
    format!(
        r#"#!/bin/sh
# Written by `hexscope menu`: a clean copy of each file chosen, beside it.
out=$({} clean --verify "$@" 2>&1)
if command -v zenity >/dev/null 2>&1; then
	printf '%s\n' "$out" | zenity --text-info --title=hexscope --width=640 --height=480
elif command -v notify-send >/dev/null 2>&1; then
	notify-send hexscope "$(printf '%s\n' "$out" | tail -n 1)"
fi
"#,
        sh_quote(exe)
    )
}

fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{NAME}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.item</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

/// An Automator Quick Action of one step, Run Shell Script, taking Finder's
/// selection as arguments.
fn workflow(script: &str) -> String {
    let argument = |n: u8, name: &str, default: &str| {
        format!(
            "\t\t\t\t\t<key>{n}</key>\n\t\t\t\t\t<dict>\n\t\t\t\t\t\t<key>default value</key>\n\t\t\t\t\t\t{default}\n\t\t\t\t\t\t<key>name</key>\n\t\t\t\t\t\t<string>{name}</string>\n\t\t\t\t\t\t<key>required</key>\n\t\t\t\t\t\t<string>0</string>\n\t\t\t\t\t\t<key>type</key>\n\t\t\t\t\t\t<string>0</string>\n\t\t\t\t\t\t<key>uuid</key>\n\t\t\t\t\t\t<string>{n}</string>\n\t\t\t\t\t</dict>\n"
        )
    };
    let arguments = [
        argument(0, "inputMethod", "<integer>0</integer>"),
        argument(1, "CheckedForUserDefaultShell", "<false/>"),
        argument(2, "source", "<string></string>"),
        argument(3, "COMMAND_STRING", "<string></string>"),
        argument(4, "shell", "<string>/bin/sh</string>"),
    ]
    .concat();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>534</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{script}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/zsh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>5E1B3C2A-6F0D-4C8E-9B47-2A1D8E6F3C01</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
					<string>Command</string>
					<string>Run</string>
					<string>Unix</string>
				</array>
				<key>OutputUUID</key>
				<string>5E1B3C2A-6F0D-4C8E-9B47-2A1D8E6F3C02</string>
				<key>UUID</key>
				<string>5E1B3C2A-6F0D-4C8E-9B47-2A1D8E6F3C03</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict>
{arguments}				</dict>
				<key>conversionLabel</key>
				<integer>0</integer>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:305.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#,
        script = xml_escape(script)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_quotes_spaces_and_percents_stays_one_path() {
        assert_eq!(sh_quote("/a b/it's"), r"'/a b/it'\''s'");
        let cmd = send_to_script(r"C:\Users\Ann 100%\hexscope.exe");
        assert!(
            cmd.contains(r#""C:\Users\Ann 100%%\hexscope.exe" clean --verify %*"#),
            "{cmd}"
        );
        assert!(cmd.ends_with("pause\r\n"));
        let script = files_script("/opt/my tools/hexscope");
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(r#"out=$('/opt/my tools/hexscope' clean --verify "$@" 2>&1)"#));
    }

    #[test]
    fn the_quick_action_holds_the_script_escaped_and_answers_to_finder() {
        let wflow = workflow(&finder_script("/Users/ann/R&D <x>/hexscope"));
        assert!(wflow.contains("'/Users/ann/R&amp;D &lt;x&gt;/hexscope' clean --verify"));
        assert!(wflow.contains("<string>/bin/zsh</string>"));
        assert!(wflow.contains("com.apple.Automator.servicesMenu"));
        assert!(info_plist().contains("<string>Clean with hexscope</string>"));
        assert!(info_plist().contains("<string>com.apple.finder</string>"));
    }

    #[test]
    fn homebrew_s_versioned_folder_is_traded_for_its_link() {
        let dir = std::env::temp_dir().join(format!("hexscope-menu-{}", std::process::id()));
        let cellar = dir.join("Cellar/hexscope/0.7.0/bin/hexscope");
        std::fs::create_dir_all(cellar.parent().unwrap()).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/hexscope"), b"").unwrap();
        assert_eq!(stable(&cellar), Some(dir.join("bin/hexscope")));
        assert_eq!(stable(Path::new("/usr/local/bin/hexscope")), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
