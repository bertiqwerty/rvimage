use std::{
    fmt::Debug,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
};

use rvimage_domain::{rverr, to_rv, RvError, RvResult};
use ssh2::{Channel, Session};

use crate::cfg::SshCfg;

fn to_cmd_err<E>(cmd: &str, e: E) -> RvError
where
    E: Debug,
{
    rverr!("could not run {} due to {:?}", cmd, e)
}

fn command(cmd: &str, sess: &Session) -> RvResult<String> {
    let mut channel = sess.channel_session().map_err(|e| to_cmd_err(cmd, e))?;

    let mut s = String::new();
    channel.exec(cmd).map_err(|e| to_cmd_err(cmd, e))?;

    channel
        .read_to_string(&mut s)
        .map_err(|e| to_cmd_err(cmd, e))?;
    channel.wait_close().map_err(|e| to_cmd_err(cmd, e))?;
    Ok(s)
}

pub fn file_info(path: &str, sess: &Session) -> RvResult<String> {
    let cmd = format!("ls -lh {path}");
    command(cmd.as_str(), sess)
}

fn close(mut remote_file: Channel) -> RvResult<()> {
    remote_file.send_eof().map_err(to_rv)?;
    remote_file.wait_eof().map_err(to_rv)?;
    remote_file.close().map_err(to_rv)?;
    remote_file.wait_close().map_err(to_rv)?;
    Ok(())
}

pub fn download(remote_src_file_path: &str, sess: &Session) -> RvResult<Vec<u8>> {
    let (mut remote_file, _) = sess
        .scp_recv(Path::new(remote_src_file_path))
        .map_err(to_rv)?;
    {
        let mut content = vec![];
        remote_file.read_to_end(&mut content).map_err(to_rv)?;
        close(remote_file)?;
        Ok(content)
    }
    .map_err(|e: RvError| rverr!("could not download {} due to {e:?}", remote_src_file_path))
}

pub fn write_bytes(content_bytes: &[u8], remote_dst_path: &Path, sess: &Session) -> RvResult<()> {
    let n_bytes = content_bytes.len();
    let mut remote_file = sess
        .scp_send(
            Path::new(remote_dst_path),
            0o644,
            content_bytes.len() as u64,
            None,
        )
        .map_err(to_rv)?;
    let mut total_bytes_written = 0;
    while total_bytes_written < n_bytes {
        total_bytes_written += remote_file
            .write(&content_bytes[total_bytes_written..])
            .map_err(|e| rverr!("could not write to {remote_dst_path:?} due to {e:?}"))?;
    }
    close(remote_file)?;
    Ok(())
}

pub fn find(
    remote_folder_path: &str,
    filter_extensions: &[&str],
    sess: &Session,
) -> RvResult<Vec<String>> {
    let cmd = format!("find '{remote_folder_path}'");
    let s = command(cmd.as_str(), sess)?;
    fn ext_predicate(s: &str, filter_extensions: &[&str]) -> bool {
        let n_s = s.len();
        filter_extensions
            .iter()
            .filter(|ext| {
                let n_e = ext.len();
                if n_e > n_s {
                    false
                } else {
                    &s[n_s - n_e..n_s] == **ext
                }
            })
            .count()
            > 0
            || filter_extensions.is_empty()
    }
    Ok(s.split('\n')
        .filter(|s| ext_predicate(s, filter_extensions))
        .map(|s| s.to_string())
        .collect::<Vec<_>>())
}

pub fn auth(ssh_cfg: &SshCfg) -> RvResult<Session> {
    let tcp = TcpStream::connect(&ssh_cfg.prj.address)
        .map_err(|e| rverr!("TCP stream connection error, {:?}", e))?;
    let mut sess = Session::new()
        .map_err(|e| rverr!("could not create ssh session, {:?}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| rverr!("ssh handshake error, {:?}", e))?;
    let keyfile = Path::new(&ssh_cfg.usr.ssh_identity_file_path);
    if !keyfile.exists() {
        return Err(rverr!("could not find private key file {keyfile:?}"));
    }
    sess.userauth_pubkey_file(&ssh_cfg.usr.user, None, keyfile, None)
        .map_err(|e| rverr!("ssh user auth error, {:?}", e))?;
    assert!(sess.authenticated());
    tracing::info!("ssh session authenticated");
    Ok(sess)
}
