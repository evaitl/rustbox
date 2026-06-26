#[cfg(applet_basename)]
pub mod basename;
#[cfg(applet_cat)]
pub mod cat;
#[cfg(applet_chmod)]
pub mod chmod;
#[cfg(applet_chown)]
pub mod chown;
#[cfg(applet_cp)]
pub mod cp;
#[cfg(applet_cron)]
pub mod cron;
#[cfg(applet_cut)]
pub mod cut;
#[cfg(applet_date)]
pub mod date;
#[cfg(applet_dd)]
pub mod dd;
#[cfg(all(applet_dig, feature = "applet-dig", target_os = "linux"))]
pub mod dig;
#[cfg(applet_dirname)]
pub mod dirname;
#[cfg(applet_dmesg)]
pub mod dmesg;
#[cfg(all(applet_dnscached, feature = "applet-dnscached", target_os = "linux"))]
pub mod dnscached;
#[cfg(applet_echo)]
pub mod echo;
#[cfg(applet_env)]
pub mod env;
#[cfg(applet_false)]
pub mod false_;
#[cfg(applet_find)]
pub mod find;
#[cfg(all(applet_free, target_os = "linux"))]
pub mod free;
#[cfg(applet_grep)]
pub mod grep;
#[cfg(applet_gzip)]
pub mod gzip;
#[cfg(applet_halt)]
pub mod halt;
#[cfg(applet_head)]
pub mod head;
#[cfg(all(applet_hostname, target_os = "linux"))]
pub mod hostname;
#[cfg(all(applet_ifconfig, target_os = "linux"))]
pub mod ifconfig;
#[cfg(applet_init)]
pub mod init;
#[cfg(applet_kill)]
pub mod kill;
#[cfg(all(applet_killall, target_os = "linux"))]
pub mod killall;
#[cfg(applet_ln)]
pub mod ln;
#[cfg(all(applet_logger, target_os = "linux"))]
pub mod logger;
#[cfg(applet_logrotate)]
pub mod logrotate;
#[cfg(applet_ls)]
pub mod ls;
#[cfg(all(applet_mdev, target_os = "linux"))]
pub mod mdev;
#[cfg(applet_mkdir)]
pub mod mkdir;
#[cfg(applet_mknod)]
pub mod mknod;
#[cfg(applet_mount)]
pub mod mount;
#[cfg(applet_mv)]
pub mod mv;
#[cfg(all(applet_nc, target_os = "linux"))]
pub mod nc;
#[cfg(all(applet_ntpclient, target_os = "linux"))]
pub mod ntpclient;
#[cfg(all(applet_passwd, feature = "applet-passwd"))]
pub mod passwd;
#[cfg(all(applet_ping, target_os = "linux"))]
pub mod ping;
#[cfg(applet_pivot_root)]
pub mod pivot_root;
#[cfg(applet_printenv)]
pub mod printenv;
#[cfg(applet_printf)]
pub mod printf;
#[cfg(applet_ps)]
pub mod ps;
#[cfg(applet_pwd)]
pub mod pwd;
#[cfg(applet_readlink)]
pub mod readlink;
#[cfg(applet_reboot)]
pub mod reboot;
#[cfg(applet_rm)]
pub mod rm;
#[cfg(applet_rmdir)]
pub mod rmdir;
#[cfg(all(applet_route, target_os = "linux"))]
pub mod route;
#[cfg(applet_sed)]
pub mod sed;
#[cfg(applet_sh)]
pub mod sh;
#[cfg(applet_sleep)]
pub mod sleep;
#[cfg(applet_sort)]
pub mod sort;
#[cfg(all(applet_sshd, feature = "applet-sshd", target_os = "linux"))]
pub mod sshd;
#[cfg(applet_stat)]
pub mod stat_;
#[cfg(applet_su)]
pub mod su;
#[cfg(all(applet_swapoff, target_os = "linux"))]
pub mod swapoff;
#[cfg(all(applet_swapon, target_os = "linux"))]
pub mod swapon;
#[cfg(applet_switch_root)]
pub mod switch_root;
#[cfg(applet_sync)]
pub mod sync;
#[cfg(all(applet_sysctl, target_os = "linux"))]
pub mod sysctl;
#[cfg(all(applet_syslogd, target_os = "linux"))]
pub mod syslogd;
#[cfg(applet_tail)]
pub mod tail;
#[cfg(applet_tar)]
pub mod tar;
#[cfg(all(applet_telnetd, target_os = "linux"))]
pub mod telnetd;
#[cfg(applet_test)]
pub mod test_;
#[cfg(all(applet_thttpd, target_os = "linux"))]
pub mod thttpd;
#[cfg(all(applet_top, target_os = "linux"))]
pub mod top;
#[cfg(applet_tr)]
pub mod tr;
#[cfg(applet_true)]
pub mod true_;
#[cfg(all(applet_udhcpc, target_os = "linux"))]
pub mod udhcpc;
#[cfg(applet_umount)]
pub mod umount;
#[cfg(applet_uname)]
pub mod uname;
#[cfg(all(applet_uptime, target_os = "linux"))]
pub mod uptime;
#[cfg(applet_vi)]
pub mod vi;
#[cfg(applet_wc)]
pub mod wc;
#[cfg(all(applet_wget, target_os = "linux"))]
pub mod wget;
#[cfg(applet_xargs)]
pub mod xargs;
