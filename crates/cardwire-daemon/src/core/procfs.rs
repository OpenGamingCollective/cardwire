//! /proc filesystem helpers for process discovery.

use std::{
    ffi::OsStr, fs, io, path::{Path, PathBuf}
};

/// read fd link to find which apps opened the gpu
pub fn lsof_read(device_path: &str) -> io::Result<Vec<String>> {
    let proc_path = Path::new("/proc");
    let mut proc_found: Vec<String> = Vec::new();
    // If proc path doesn't exist, exit
    if !proc_path.exists() || !proc_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "couldn't find /proc path",
        ));
    }
    // read /proc
    for entry in fs::read_dir(proc_path)
        .map_err(|e| io::Error::other(e.to_string()))?
        .flatten()
    {
        // Check if folder name is a numerical, if not skip
        if let Ok(string) = entry.file_name().into_string()
            && string.parse::<u32>().is_err()
        {
            continue;
        }
        let path = entry.path();
        // now read eg: /proc/1
        if path.is_dir() {
            // now get fd directory
            let fd_dir: PathBuf = fs::read_dir(&path)
                .map_err(|e| io::Error::other(e.to_string()))?
                .flatten()
                .map(|r| r.path())
                .filter(|r| r.file_name() == Some(OsStr::new("fd")))
                .collect();
            for entry in fs::read_dir(fd_dir)
                .map_err(|e| io::Error::other(e.to_string()))?
                .flatten()
            {
                if let Ok(link) = entry.path().read_link()
                    && let Some(file) = link.to_str()
                {
                    let file = file.to_string();
                    if file.contains(device_path) {
                        // Found the file, now get process name
                        let comm_read = fs::read_to_string(path.join("comm"));
                        let mut process_name: String = String::new();
                        if let Ok(comm) = comm_read {
                            process_name = comm.trim_ascii_end().to_string()
                        }
                        proc_found.push(process_name);
                    }
                }
            }
        }
    }
    Ok(proc_found)
}
