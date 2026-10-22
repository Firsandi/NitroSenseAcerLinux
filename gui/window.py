#!/usr/bin/env python3
import sys
import os

def launch_native_window(url):
    try:
        import gi
        gi.require_version('Gtk', '3.0')
        gi.require_version('WebKit2', '4.1')
        from gi.repository import Gtk, WebKit2, GLib, GdkPixbuf, Gdk

        GLib.set_prgname('nitrosense')
        GLib.set_application_name('Acer NitroSense')

        window = Gtk.Window(title='Acer NitroSense - Control Center')
        window.set_default_size(1366, 860)
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
        wk.set_enable_fullscreen(True)

        def on_enter_fullscreen(view):
            window.fullscreen()
            return True

        def on_leave_fullscreen(view):
            window.unfullscreen()
            return True

        webview.connect('enter-fullscreen', on_enter_fullscreen)
        webview.connect('leave-fullscreen', on_leave_fullscreen)

        def on_key_press(win, event):
            if event.keyval == Gdk.KEY_F11:
                gdk_win = win.get_window()
                state = gdk_win.get_state() if gdk_win else 0
                if state & Gdk.WindowState.FULLSCREEN:
                    win.unfullscreen()
                else:
                    win.fullscreen()
                return True
            return False

        window.connect('key-press-event', on_key_press)

        webview.load_uri(url)
        window.add(webview)
        window.connect('destroy', Gtk.main_quit)

        # Start maximized to fill the screen seamlessly
        window.show_all()
        window.maximize()
        GLib.idle_add(window.maximize)
        Gtk.main()
        return True
    except Exception as e:
        print(f"Native GTK WebKit error: {e}", file=sys.stderr)
        return False

if __name__ == '__main__':
    target_url = sys.argv[1] if len(sys.argv) > 1 else 'http://127.0.0.1:54433'
    if not launch_native_window(target_url):
        sys.exit(1)
