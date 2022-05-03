use std::{io::Read, net::TcpStream, path::Path};

use ssh2::Session;

use crate::{
    cfg::SshCfg,
    result::{to_rv, RvResult},
};

pub fn download(remote_src_file_path: &str, sess: &Session) -> RvResult<Vec<u8>> {
    let (mut remote_file, _) = sess
        .scp_recv(Path::new(remote_src_file_path))
        .map_err(to_rv)?;
    let mut content = vec![];
    remote_file.read_to_end(&mut content).map_err(to_rv)?;
    remote_file.send_eof().map_err(to_rv)?;
    remote_file.wait_eof().map_err(to_rv)?;
    remote_file.close().map_err(to_rv)?;
    remote_file.wait_close().map_err(to_rv)?;
    Ok(content)
}

pub fn find(ssh_cfg: &SshCfg, filter_extensions: &[&str]) -> RvResult<Vec<String>> {
    let sess = auth(ssh_cfg)?;
    let mut channel = sess.channel_session().map_err(to_rv)?;

    let mut s = String::new();
    channel
        .exec(format!("find {}", ssh_cfg.remote_folder_path).as_str())
        .map_err(to_rv)?;

    channel.read_to_string(&mut s).map_err(to_rv)?;
    channel.wait_close().map_err(to_rv)?;
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
    let tcp = TcpStream::connect(&ssh_cfg.address).map_err(to_rv)?;
    let mut sess = Session::new().map_err(to_rv)?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(to_rv)?;
    sess.userauth_pubkey_file(
        &ssh_cfg.user,
        None,
        Path::new(&ssh_cfg.ssh_identity_file_path),
        None,
    )
    .map_err(to_rv)?;
    assert!(sess.authenticated());
    Ok(sess)
}
