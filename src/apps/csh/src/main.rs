#![no_std]
#![no_main]

use cube_syscall::*;

const MAX_LINE: usize = 256;

fn print(s: &[u8]) { write(STDOUT, s); }
fn eprint(s: &[u8]) { write(STDERR, s); }

fn read_line(buf: &mut [u8]) -> usize {
    let n = read(STDIN, buf);
    if n <= 0 { return 0; }
    let mut len = n as usize;
    while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') { len -= 1; }
    len
}

fn cmd_ls(args: &[u8]) {
    let path: &[u8] = if args.is_empty() { b"." } else { args };
    let fd = open(path, O_RDONLY | O_DIRECTORY);
    if fd < 0 { eprint(b"ls: cannot open\n"); return; }

    let mut buf = [0u8; 1024];
    let n = getdents(fd as usize, &mut buf);
    close(fd as usize);
    if n < 0 { eprint(b"ls: getdents failed\n"); return; }

    let total = n as usize;
    let mut off = 0usize;
    while off + 19 <= total {
        let reclen = u16::from_le_bytes([buf[off + 16], buf[off + 17]]) as usize;
        if reclen == 0 { break; }
        let ftype = buf[off + 18];
        let mut name_len = 0;
        while off + 19 + name_len < total && buf[off + 19 + name_len] != 0 {
            name_len += 1;
        }
        let name = &buf[off + 19..off + 19 + name_len];
        if name != b"." && name != b".." {
            print(name);
            if ftype == DT_DIR { print(b"/"); }
            print(b"\n");
        }
        off += reclen;
    }
}

fn cmd_cd(args: &[u8]) {
    let path: &[u8] = if args.is_empty() { b"/" } else { args };
    if chdir(path) < 0 { eprint(b"cd: no such directory\n"); }
}

fn cmd_pwd(_args: &[u8]) {
    let mut buf = [0u8; 256];
    let n = getcwd(&mut buf);
    if n > 0 {
        print(&buf[..n as usize]);
        print(b"\n");
    }
}

fn cmd_cat(args: &[u8]) {
    if args.is_empty() { eprint(b"cat: missing argument\n"); return; }
    let fd = open(args, O_RDONLY);
    if fd < 0 { eprint(b"cat: cannot open\n"); return; }
    let mut buf = [0u8; 512];
    loop {
        let n = read(fd as usize, &mut buf);
        if n <= 0 { break; }
        write(STDOUT, &buf[..n as usize]);
    }
    close(fd as usize);
}

fn cmd_mkdir(args: &[u8]) {
    if args.is_empty() { eprint(b"mkdir: missing argument\n"); return; }
    if mkdir(args, 0o755) < 0 { eprint(b"mkdir: failed\n"); }
}

fn cmd_rm(args: &[u8]) {
    if args.is_empty() { eprint(b"rm: missing argument\n"); return; }
    if unlink(args) < 0 { eprint(b"rm: failed\n"); }
}

fn cmd_echo(args: &[u8]) {
    print(args);
    print(b"\n");
}

fn cmd_help(_args: &[u8]) {
    print(b"Commands:\n");
    print(b"  ls [path]     list directory\n");
    print(b"  cd [path]     change directory\n");
    print(b"  pwd           print working directory\n");
    print(b"  cat <file>    print file contents\n");
    print(b"  mkdir <path>  create directory\n");
    print(b"  rm <path>     remove file\n");
    print(b"  echo <text>   print text\n");
    print(b"  help          this message\n");
    print(b"  exit          quit shell\n");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    print(b"CubeOS Shell v0.1\n");
    print(b"Type 'help' for commands.\n\n");

    let mut line = [0u8; MAX_LINE];
    loop {
        let mut cwd = [0u8; 256];
        let cn = getcwd(&mut cwd);
        if cn > 0 { print(&cwd[..cn as usize]); }
        print(b"$ ");

        let len = read_line(&mut line);
        if len == 0 { continue; }

        let mut i = 0;
        while i < len && line[i] == b' ' { i += 1; }
        let cmd_start = i;
        while i < len && line[i] != b' ' { i += 1; }
        let cmd = &line[cmd_start..i];
        while i < len && line[i] == b' ' { i += 1; }
        let args = &line[i..len];

        match cmd {
            b"ls"    => cmd_ls(args),
            b"cd"    => cmd_cd(args),
            b"pwd"   => cmd_pwd(args),
            b"cat"   => cmd_cat(args),
            b"mkdir" => cmd_mkdir(args),
            b"rm"    => cmd_rm(args),
            b"echo"  => cmd_echo(args),
            b"help"  => cmd_help(args),
            b"exit"  => { print(b"bye\n"); exit(0); }
            b""      => {}
            _ => {
                eprint(b"unknown command: ");
                eprint(cmd);
                eprint(b"\n");
            }
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    eprint(b"[csh] panic\n");
    loop {}
}
