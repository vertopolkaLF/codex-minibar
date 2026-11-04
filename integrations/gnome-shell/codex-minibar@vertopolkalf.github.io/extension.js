// Codex Minibar top-bar indicator.
//
// A thin client of the app's session D-Bus service
// (io.github.vertopolkalf.CodexMinibar1, see src/linux/dbus.rs): it shows the
// tray's default quotas of each enabled provider as the app writes them, in
// the app's language and remaining/used preference, and updates on the
// service's Changed signal. The indicator hides while the app is not running.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const BUS_NAME = 'io.github.vertopolkalf.CodexMinibar';
const OBJECT_PATH = '/io/github/vertopolkalf/CodexMinibar';
const INTERFACE_XML = `
<node>
  <interface name="io.github.vertopolkalf.CodexMinibar1">
    <method name="Status">
      <arg type="s" direction="out"/>
    </method>
    <method name="Toggle">
      <arg name="activation_token" type="s" direction="in"/>
    </method>
    <method name="ShowProvider">
      <arg name="id" type="s" direction="in"/>
    </method>
    <method name="Refresh"/>
    <signal name="Changed">
      <arg name="status" type="s"/>
    </signal>
  </interface>
</node>`;
const MinibarProxy = Gio.DBusProxy.makeProxyWrapper(INTERFACE_XML);

const WARNING_PERCENT = 80;
const CRITICAL_PERCENT = 95;

function levelClass(percent) {
    if (typeof percent !== 'number')
        return '';
    if (percent >= CRITICAL_PERCENT)
        return 'codex-minibar-critical';
    if (percent >= WARNING_PERCENT)
        return 'codex-minibar-warning';
    return '';
}

function quotaLine(quota) {
    const parts = [`${quota.label}: ${quota.value_text}`];
    if (quota.reset_in)
        parts.push(quota.reset_in);
    return parts.join(' · ');
}

const MinibarIndicator = GObject.registerClass(
class MinibarIndicator extends PanelMenu.Button {
    _init() {
        super._init(0.0, 'Codex Minibar');
        this._proxy = null;
        this._changedId = 0;
        this._cancellable = null;

        this._box = new St.BoxLayout({style_class: 'panel-status-menu-box codex-minibar-box'});
        this.add_child(this._box);

        this._providers = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._providers);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction('Open Codex Minibar', () => this._proxy?.ToggleRemote('', () => {}));
        this.menu.addAction('Refresh', () => this._proxy?.RefreshRemote(() => {}));

        this.visible = false;
        this._watchId = Gio.bus_watch_name(
            Gio.BusType.SESSION,
            BUS_NAME,
            Gio.BusNameWatcherFlags.NONE,
            () => this._connect(),
            () => this._disconnect());
    }

    _connect() {
        this._disconnect();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        // eslint-disable-next-line no-new
        new MinibarProxy(
            Gio.DBus.session,
            BUS_NAME,
            OBJECT_PATH,
            (proxy, error) => {
                if (error || cancellable.is_cancelled())
                    return;
                this._proxy = proxy;
                let receivedChange = false;
                this._changedId = proxy.connectSignal('Changed',
                    (_proxy, _sender, [status]) => {
                        if (this._proxy !== proxy)
                            return;
                        receivedChange = true;
                        this._render(status);
                    });
                proxy.StatusRemote(([status], statusError) => {
                    // A Changed signal may already contain a newer snapshot.
                    if (!statusError && this._proxy === proxy && !receivedChange)
                        this._render(status);
                });
            },
            cancellable,
            Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    _disconnect() {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._proxy && this._changedId)
            this._proxy.disconnectSignal(this._changedId);
        this._proxy = null;
        this._changedId = 0;
        this.visible = false;
    }

    _render(json) {
        let report;
        try {
            report = JSON.parse(json);
        } catch {
            return;
        }
        const providers = Array.isArray(report?.providers) ? report.providers : [];

        this._box.destroy_all_children();
        this._providers.removeAll();

        providers.forEach((provider, index) => {
            const iconPath = provider.icon?.dark;
            const gap = index > 0 ? ' codex-minibar-gap' : '';
            if (iconPath) {
                this._box.add_child(new St.Icon({
                    gicon: new Gio.FileIcon({file: Gio.File.new_for_path(iconPath)}),
                    style_class: `codex-minibar-icon${gap}`,
                    y_align: Clutter.ActorAlign.CENTER,
                }));
            }
            const level = levelClass(provider.max_used_percent);
            this._box.add_child(new St.Label({
                text: provider.headline || provider.name,
                style_class: `codex-minibar-label ${level}${iconPath ? '' : gap}`,
                y_align: Clutter.ActorAlign.CENTER,
            }));

            const title = provider.plan ? `${provider.name} · ${provider.plan}` : provider.name;
            const item = new PopupMenu.PopupMenuItem(title);
            item.connect('activate', () => this._proxy?.ShowProviderRemote(provider.id, () => {}));
            this._providers.addMenuItem(item);
            for (const quota of provider.quotas ?? []) {
                this._providers.addMenuItem(new PopupMenu.PopupMenuItem(quotaLine(quota), {
                    reactive: false,
                    style_class: 'codex-minibar-quota',
                }));
            }
            if (provider.today) {
                this._providers.addMenuItem(new PopupMenu.PopupMenuItem(provider.today, {
                    reactive: false,
                    style_class: 'codex-minibar-quota',
                }));
            }
        });

        if (providers.length === 0) {
            this._box.add_child(new St.Label({
                text: 'Minibar',
                style_class: 'codex-minibar-label',
                y_align: Clutter.ActorAlign.CENTER,
            }));
        }
        this.visible = true;
    }

    destroy() {
        if (this._watchId) {
            Gio.bus_unwatch_name(this._watchId);
            this._watchId = 0;
        }
        this._disconnect();
        super.destroy();
    }
});

export default class CodexMinibarExtension extends Extension {
    enable() {
        this._indicator = new MinibarIndicator();
        Main.panel.addToStatusArea(this.uuid, this._indicator);
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
    }
}
