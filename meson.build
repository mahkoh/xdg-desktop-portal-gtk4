project('xdg-desktop-portal-gtk4')

prefix = get_option('prefix')
datadir = prefix / get_option('datadir')
libexecdir = prefix / get_option('libexecdir')
dbus_service_dir = get_option('dbus-service-dir')
if dbus_service_dir == ''
  dbus_service_dir = prefix /  datadir / 'dbus-1' / 'services'
endif
systemd_userunit_dir = get_option('systemd-user-unit-dir')
if systemd_userunit_dir == ''
  systemd_userunit_dir = prefix / 'lib' / 'systemd' / 'user'
endif

install_data('target/release/xdg-desktop-portal-gtk4', install_dir: libexecdir)
install_data('data/gtk4.portal', install_dir: datadir / 'xdg-desktop-portal/portals')
conf = configuration_data()
conf.set('libexecdir', libexecdir)
configure_file(
  input: 'data/org.freedesktop.impl.portal.desktop.gtk4.service.in',
  output: 'org.freedesktop.impl.portal.desktop.gtk4.service',
  configuration: conf,
  install: true,
  install_dir: dbus_service_dir,
)
configure_file(
  input: 'data/xdg-desktop-portal-gtk4.service.in',
  output: 'xdg-desktop-portal-gtk4.service',
  configuration: conf,
  install: true,
  install_dir: systemd_userunit_dir,
)
configure_file(
  input: 'data/xdg-desktop-portal-gtk4.desktop.in',
  output: 'xdg-desktop-portal-gtk4.desktop',
  configuration: conf,
  install: true,
  install_dir: datadir / 'applications',
)
