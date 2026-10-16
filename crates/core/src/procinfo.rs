// ps を起動すると 1 回 40ms ほど CPU を使い、3 秒ごとや hook のたびに呼ぶと重い。
// macOS の proc_* と sysctl で、必要なプロセスの必要な情報だけを読む
const MAX_ANCESTORS: usize = 32;
// 起動コマンドの全文が要るのは claude・codex かどうかを見分けるときだけ。ほかのプロセスは読まずに軽くする
const ARG_CANDIDATES: [&str; 3] = ["claude", "node", "codex"];

// ネイティブ版の claude は ~/.local/share/claude/versions/<版> へのリンクなので、macOS の内部のプロセスの名前が版の番号になる
fn reads_args(comm: &str) -> bool {
    ARG_CANDIDATES.contains(&comm) || is_version_name(comm)
}

fn is_version_name(comm: &str) -> bool {
    let parts: Vec<&str> = comm.split('.').collect();
    parts.len() >= 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

pub fn parse_procargs2(buf: &[u8]) -> Option<String> {
    parse_procargs2_argv(buf).map(|args| args.join(" "))
}

// 引数を空白でつながずに 1 つずつ返す。依頼の文（「review the auth changes」）とサブコマンドを見分けるときに使う
pub fn parse_procargs2_argv(buf: &[u8]) -> Option<Vec<String>> {
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    let rest = &buf[4..];
    let exec_end = rest.iter().position(|b| *b == 0)?;
    let mut fields = rest[exec_end..].split(|b| *b == 0).filter(|f| !f.is_empty());
    let args: Vec<String> = (0..argc)
        .map_while(|_| fields.next())
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    (!args.is_empty()).then_some(args)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::CStr;
    use std::mem::{size_of, MaybeUninit};
    use std::sync::OnceLock;

    use super::{parse_procargs2_argv, reads_args, MAX_ANCESTORS};
    use crate::ps::ProcRow;

    struct BsdInfo {
        ppid: u32,
        comm: String,
        tty_dev: u32,
    }

    fn bsd_info(pid: u32) -> Option<BsdInfo> {
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = size_of::<libc::proc_bsdinfo>() as i32;
        let got = unsafe { libc::proc_pidinfo(pid as i32, libc::PROC_PIDTBSDINFO, 0, info.as_mut_ptr().cast(), size) };
        if got != size {
            return short_bsd_info(pid);
        }
        let info = unsafe { info.assume_init() };
        let comm = unsafe { CStr::from_ptr(info.pbi_comm.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Some(BsdInfo {
            ppid: info.pbi_ppid,
            comm,
            tty_dev: info.e_tdev,
        })
    }

    // root の login など、ほかのユーザーのプロセスは詳しい情報を読めない。親をたどるのに要る親の番号と名前だけは、短い形で読める
    fn short_bsd_info(pid: u32) -> Option<BsdInfo> {
        let mut info = MaybeUninit::<libc::proc_bsdshortinfo>::zeroed();
        let size = size_of::<libc::proc_bsdshortinfo>() as i32;
        let got = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDT_SHORTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if got != size {
            return None;
        }
        let info = unsafe { info.assume_init() };
        let comm = unsafe { CStr::from_ptr(info.pbsi_comm.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Some(BsdInfo {
            ppid: info.pbsi_ppid,
            comm,
            tty_dev: u32::MAX,
        })
    }

    fn tty_name(dev: u32) -> Option<String> {
        if dev == u32::MAX || dev == 0 {
            return None;
        }
        let name = unsafe { libc::devname(dev as libc::dev_t, libc::S_IFCHR) };
        (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }

    fn arg_max() -> usize {
        static ARG_MAX: OnceLock<usize> = OnceLock::new();
        *ARG_MAX.get_or_init(|| {
            let mut value: libc::c_int = 0;
            let mut size = size_of::<libc::c_int>();
            let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
            let ok = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    2,
                    (&mut value as *mut libc::c_int).cast(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            } == 0;
            if ok && value > 0 {
                value as usize
            } else {
                1 << 20
            }
        })
    }

    thread_local! {
        // KERN_ARGMAX は 1MB ほどあり、claude や node のプロセスごとに確保し直すと 3 秒ごとに数 MB を埋めることになる
        static ARGS_BUFFER: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    pub fn process_args(pid: u32) -> Option<String> {
        process_argv(pid).map(|args| args.join(" "))
    }

    pub fn process_argv(pid: u32) -> Option<Vec<String>> {
        ARGS_BUFFER.with_borrow_mut(|buf| {
            buf.resize(arg_max(), 0);
            let mut size = buf.len();
            let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
            let ok = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    3,
                    buf.as_mut_ptr().cast(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            } == 0;
            if !ok {
                return None;
            }
            parse_procargs2_argv(&buf[..size])
        })
    }

    // devname は呼ぶたびに /dev を読み直すので、端末名は親をたどるとき（hook・移動）だけ求め、3 秒ごとの一覧では求めない
    fn row(pid: u32, info: BsdInfo, args: String, with_tty: bool) -> ProcRow {
        let tty = if with_tty { tty_name(info.tty_dev) } else { None };
        ProcRow {
            pid,
            ppid: info.ppid,
            tty,
            args,
        }
    }

    pub fn list_processes() -> Option<Vec<ProcRow>> {
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if count <= 0 {
            return None;
        }
        // 数え終えてから一覧を取るまでにプロセスが増えることがあるので、余裕を持たせる
        let mut pids = vec![0 as libc::c_int; count as usize + 64];
        let bytes = (pids.len() * size_of::<libc::c_int>()) as libc::c_int;
        let filled = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        if filled <= 0 {
            return None;
        }
        pids.truncate(filled as usize);
        let rows = pids
            .into_iter()
            .filter(|pid| *pid > 0)
            .filter_map(|pid| {
                let pid = pid as u32;
                let info = bsd_info(pid)?;
                let args = if reads_args(&info.comm) {
                    process_args(pid).unwrap_or_default()
                } else {
                    String::new()
                };
                Some(row(pid, info, args, false))
            })
            .collect();
        Some(rows)
    }

    pub fn process_rows(pids: &[u32]) -> Vec<ProcRow> {
        pids.iter()
            .filter_map(|&pid| {
                let info = bsd_info(pid)?;
                let args = process_args(pid).unwrap_or_else(|| info.comm.clone());
                Some(row(pid, info, args, false))
            })
            .collect()
    }

    pub fn ancestor_rows(pid: u32) -> Vec<ProcRow> {
        let mut rows = Vec::new();
        let mut current = pid;
        for _ in 0..MAX_ANCESTORS {
            let Some(info) = bsd_info(current) else { break };
            let next = info.ppid;
            let args = process_args(current).unwrap_or_else(|| info.comm.clone());
            rows.push(row(current, info, args, true));
            if next == 0 || next == current {
                break;
            }
            current = next;
        }
        rows
    }

    pub fn working_directory(pid: u32) -> Option<String> {
        let mut info = MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
        let size = size_of::<libc::proc_vnodepathinfo>() as i32;
        let got = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDVNODEPATHINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if got != size {
            return None;
        }
        let info = unsafe { info.assume_init() };
        let path = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) }
            .to_string_lossy()
            .into_owned();
        (!path.is_empty()).then_some(path)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::ps::ProcRow;

    pub fn list_processes() -> Option<Vec<ProcRow>> {
        None
    }
    pub fn ancestor_rows(_pid: u32) -> Vec<ProcRow> {
        Vec::new()
    }
    pub fn process_rows(_pids: &[u32]) -> Vec<ProcRow> {
        Vec::new()
    }
    pub fn process_argv(_pid: u32) -> Option<Vec<String>> {
        None
    }
    pub fn working_directory(_pid: u32) -> Option<String> {
        None
    }
}

pub use imp::{ancestor_rows, list_processes, process_argv, process_rows, working_directory};

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> u32 {
        std::process::id()
    }

    #[test]
    fn lists_this_process_with_its_parent() {
        let rows = list_processes().expect("プロセスの一覧が取れること");
        let row = rows.iter().find(|r| r.pid == me()).expect("自分が一覧にあること");
        assert_eq!(row.ppid, std::os::unix::process::parent_id());
        assert!(rows.len() > 10);
    }

    #[test]
    fn ancestors_start_with_this_process_and_carry_full_args() {
        let rows = ancestor_rows(me());
        assert_eq!(rows[0].pid, me());
        let exe = std::env::current_exe().unwrap();
        assert!(rows[0].args.starts_with(exe.to_str().unwrap()), "{}", rows[0].args);
        assert_eq!(rows.get(1).map(|r| r.pid), Some(std::os::unix::process::parent_id()));
    }

    #[test]
    fn reads_the_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(working_directory(me()).as_deref(), Some(cwd.to_str().unwrap()));
        assert_eq!(working_directory(u32::MAX), None);
    }

    #[test]
    fn reads_only_the_requested_processes() {
        let rows = process_rows(&[me(), u32::MAX]);
        assert_eq!(rows.len(), 1, "ないプロセスは返さない");
        assert_eq!(rows[0].pid, me());
        assert_eq!(rows[0].ppid, std::os::unix::process::parent_id());
        assert!(rows[0]
            .args
            .starts_with(std::env::current_exe().unwrap().to_str().unwrap()));
        assert_eq!(rows[0].tty, None);
    }

    #[test]
    fn keeps_each_argument_separate() {
        let mut buf = 2i32.to_ne_bytes().to_vec();
        buf.extend_from_slice(
            b"/opt/homebrew/bin/codex\0\0/opt/homebrew/bin/codex\0review the auth changes\0HOME=/u\0",
        );
        assert_eq!(
            parse_procargs2_argv(&buf),
            Some(vec![
                "/opt/homebrew/bin/codex".to_string(),
                "review the auth changes".to_string()
            ])
        );
        let me = std::process::id();
        assert!(process_argv(me).is_some_and(|argv| argv[0].contains("agent_loaf_core")));
    }

    #[test]
    fn parses_kern_procargs2_buffers() {
        let mut buf = 3i32.to_ne_bytes().to_vec();
        buf.extend_from_slice(b"/usr/bin/node\0\0\0node\0/opt/claude-code/cli.js\0--resume\0HOME=/u\0");
        assert_eq!(
            parse_procargs2(&buf).as_deref(),
            Some("node /opt/claude-code/cli.js --resume")
        );
        assert_eq!(parse_procargs2(&[1, 0]), None);
    }

    #[test]
    fn reads_full_args_for_every_agent_process() {
        // 全文を読まないと、生きているかの確認で Claude Code・Codex のプロセスと見分けられず、会話を終わりにしてしまう
        for comm in ["claude", "node", "codex", "2.1.296", "2.1.300"] {
            assert!(reads_args(comm), "{comm}");
        }
        for comm in ["zsh", "login", "1.2", "2.1.", "v2.1.296", "2.1.x"] {
            assert!(!reads_args(comm), "{comm}");
        }
    }

    // 下のテストが別のプロセスとして動かす。Apple の実行ファイル（/bin/sleep など）を写して動かすと macOS に止められるため、このテストの実行ファイルを使う
    #[test]
    #[ignore]
    fn waits_as_a_stand_in_process() {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }

    #[test]
    fn finds_claude_installed_by_the_native_installer() {
        // ネイティブ版の claude は versions/<版> へのリンクなので、macOS の内部のプロセスの名前は版の番号（2.1.296 など）になる
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("2.1.296");
        std::fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
        let link = dir.path().join("claude");
        std::os::unix::fs::symlink(&binary, &link).unwrap();
        let mut child = std::process::Command::new(&link)
            .args(["--ignored", "--exact", "procinfo::tests::waits_as_a_stand_in_process"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let rows = list_processes().unwrap();
        let row = rows.iter().find(|r| r.pid == child.id()).cloned();
        child.kill().unwrap();
        child.wait().unwrap();
        let row = row.expect("一覧に出る");
        assert!(
            crate::ps::is_agent_command(&row.args),
            "引数を読めていない: {:?}",
            row.args
        );
    }

    #[test]
    fn ancestors_continue_through_processes_owned_by_root() {
        // ターミナルでは claude・codex の親に root の login が挟まる。読めずに止まると、その先の Terminal.app にたどり着けない
        let rows = ancestor_rows(1);
        assert_eq!(
            rows.first().map(|r| r.pid),
            Some(1),
            "root の launchd も親子関係として読める"
        );
        assert_eq!(rows.first().map(|r| r.ppid), Some(0));
    }
}
