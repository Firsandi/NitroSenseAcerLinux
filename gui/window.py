#!/usr/bin/env python3
import sys
import os

def launch_native_window(url):
    try:
        import gi
        gi.require_version('Gtk', '3.0')
        gi.require_version('WebKit2', '4.1')
        from gi.repository import Gtk, WebKit2, GLib, GdkPixbuf

        GLib.set_prgname('nitrosense')
        GLib.set_application_name('Acer NitroSense')

        window = Gtk.Window(title='Acer NitroSense - Control Center')
        window.set_default_size(1240, 820)
        window.set_position(Gtk.WindowPosition.CENTER)

        # Set custom NitroSense icon
        icon_paths = [
            os.path.expanduser('~/.local/share/icons/nitrosense.svg'),
            os.path.join(os.path.dirname(os.path.abspath(__file__)), 'nitrosense.svg'),
            '/home/firsandia/Desktop/ProjekFomo1/nitrosense-linux/gui/nitrosense.svg',
        ]
        for p in icon_paths:
            if os.path.exists(p):
                try:
                    pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(p, 64, 64, True)
                    window.set_icon(pixbuf)
                    break
                except Exception:
                    pass

        # Prefer sleek dark theme
        settings = Gtk.Settings.get_default()
        if settings:
            settings.set_property('gtk-application-prefer-dark-theme', True)

        webview = WebKit2.WebView()
        wk = webview.get_settings()
        wk.set_enable_javascript(True)
        wk.set_enable_webgl(True)

        webview.load_uri(url)
        window.add(webview)
        window.connect('destroy', Gtk.main_quit)
        window.show_all()
        Gtk.main()
        return True
    except Exception as e:
        print(f"Native GTK WebKit error: {e}", file=sys.stderr)
        return False

if __name__ == '__main__':
    target_url = sys.argv[1] if len(sys.argv) > 1 else 'http://127.0.0.1:54433'
    if not launch_native_window(target_url):
        sys.exit(1)
