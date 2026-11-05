// Codex Minibar panel widget for Plasma 6.
//
// Reads the app's D-Bus status (with a configured CLI fallback) on a timer and
// shows each enabled provider's default quotas as the app writes them, in the
// app's language and remaining/used preference. Clicking a provider opens the
// app's popup on its tab through the app's D-Bus service.

import QtQuick
import QtQuick.Layouts

import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as P5Support
import org.kde.plasma.plasmoid

PlasmoidItem {
    id: root

    readonly property string busName: "io.github.vertopolkalf.CodexMinibar"
    readonly property string objectPath: "/io/github/vertopolkalf/CodexMinibar"
    readonly property string interfaceName: "io.github.vertopolkalf.CodexMinibar1"
    readonly property string executable: Plasmoid.configuration.executable || "codex-minibar"
    readonly property string statusCommand: statusCommandFor(executable)
    readonly property int pollSeconds: Math.max(5, Plasmoid.configuration.intervalSeconds)
    property bool ready: false

    onStatusCommandChanged: {
        if (ready) refreshNow();
    }
    onPollSecondsChanged: {
        if (ready) refreshNow();
    }
    Component.onCompleted: {
        ready = true;
        refreshNow();
    }

    property var providers: []
    property bool running: false
    property string error: ""
    readonly property bool darkPanel: Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark

    Plasmoid.status: running ? PlasmaCore.Types.ActiveStatus : PlasmaCore.Types.PassiveStatus
    toolTipMainText: "Codex Minibar"
    toolTipSubText: running
        ? providers.map(provider => provider.name + "  " + provider.headline).join("\n")
        : (error || "Codex Minibar is not running")

    function levelColor(percent) {
        if (typeof percent !== "number") {
            return Kirigami.Theme.textColor;
        }
        if (percent >= 95) {
            return Kirigami.Theme.negativeTextColor;
        }
        if (percent >= 80) {
            return Kirigami.Theme.neutralTextColor;
        }
        return Kirigami.Theme.textColor;
    }

    function iconSource(provider) {
        if (!provider.icon) {
            return "";
        }
        const path = darkPanel ? provider.icon.dark : provider.icon.light;
        // Paths from the report are filesystem paths, not pre-encoded URLs.
        // Encode components so #, ? and % remain part of the filename.
        return path ? "file://" + path.split("/").map(encodeURIComponent).join("/") : "";
    }

    function shellQuote(text) {
        return "'" + String(text).replace(/'/g, "'\\''") + "'";
    }

    function statusCommandFor(command) {
        // Read the running app without mounting/extracting an AppImage per poll.
        // Literal mode prints the single JSON string without D-Bus formatting.
        return "dbus-send --session --print-reply=literal --reply-timeout=1500 --dest="
            + root.busName + " " + root.objectPath + " " + root.interfaceName + ".Status"
            + " || " + shellQuote(command) + " status --json";
    }

    function callService(method, args) {
        let command = "dbus-send --session --type=method_call --dest=" + busName + " "
            + objectPath + " " + interfaceName + "." + method;
        for (const arg of args) {
            command += " " + shellQuote("string:" + arg);
        }
        runner.connectSource(command);
    }

    function refreshNow() {
        statusSource.connectSource(root.statusCommand);
    }

    function receiveStatus(sourceName, data) {
        statusSource.disconnectSource(sourceName);
        // An older executable's response can finish after settings changed.
        if (sourceName !== root.statusCommand) return;
        const stdout = data["stdout"] || "";
        try {
            const report = JSON.parse(stdout);
            // D-Bus returns Report directly; the CLI adds its `ok` wrapper.
            const valid = report && (report.ok === undefined || report.ok === true)
                && Array.isArray(report.providers);
            root.running = !!valid;
            root.providers = valid ? report.providers : [];
            root.error = valid ? "" : ((report && report.error) || "Invalid status report");
        } catch (exception) {
            root.running = false;
            root.providers = [];
            root.error = (data["stderr"] || "").trim()
                || ("Could not run " + root.executable);
        }
    }

    P5Support.DataSource {
        id: statusSource
        engine: "executable"
        connectedSources: []
        onNewData: (sourceName, data) => root.receiveStatus(sourceName, data)
    }

    // Fire-and-forget actions.
    P5Support.DataSource {
        id: runner
        engine: "executable"
        connectedSources: []
        onNewData: (sourceName, data) => disconnectSource(sourceName)
    }

    Timer {
        interval: root.pollSeconds * 1000
        running: true
        repeat: true
        onTriggered: root.refreshNow()
    }

    compactRepresentation: MouseArea {
        id: compact

        Layout.minimumWidth: row.implicitWidth
        Layout.preferredWidth: row.implicitWidth
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton
        onClicked: mouse => {
            if (mouse.button === Qt.MiddleButton) {
                root.callService("Toggle", [""]);
            } else {
                root.expanded = !root.expanded;
            }
        }

        RowLayout {
            id: row
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.providers

                RowLayout {
                    required property var modelData
                    spacing: Kirigami.Units.smallSpacing / 2

                    Kirigami.Icon {
                        visible: source !== ""
                        source: root.iconSource(modelData)
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    }

                    PlasmaComponents.Label {
                        text: modelData.headline || modelData.name
                        color: root.levelColor(modelData.max_used_percent)
                        font.features: ({ "tnum": 1 })
                    }
                }
            }

            Kirigami.Icon {
                visible: root.providers.length === 0
                source: "codex-minibar"
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 16
        Layout.preferredWidth: Kirigami.Units.gridUnit * 20
        spacing: Kirigami.Units.largeSpacing

        PlasmaComponents.Label {
            visible: !root.running
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: root.error || "Codex Minibar is not running"
        }

        Repeater {
            model: root.providers

            ColumnLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                PlasmaComponents.ItemDelegate {
                    Layout.fillWidth: true
                    icon.source: root.iconSource(modelData)
                    text: modelData.plan ? modelData.name + " · " + modelData.plan : modelData.name
                    font.bold: true
                    onClicked: root.callService("ShowProvider", [modelData.id])
                }

                Repeater {
                    model: modelData.quotas || []

                    ColumnLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        spacing: 0

                        RowLayout {
                            Layout.fillWidth: true

                            PlasmaComponents.Label {
                                Layout.fillWidth: true
                                text: modelData.label
                                elide: Text.ElideRight
                            }

                            PlasmaComponents.Label {
                                text: modelData.reset_in
                                    ? modelData.value_text + " · " + modelData.reset_in
                                    : modelData.value_text
                                opacity: 0.8
                            }
                        }

                        PlasmaComponents.ProgressBar {
                            visible: typeof modelData.progress === "number"
                            Layout.fillWidth: true
                            from: 0
                            to: 1
                            value: typeof modelData.progress === "number" ? modelData.progress : 0
                        }
                    }
                }

                PlasmaComponents.Label {
                    visible: !!modelData.today
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    text: modelData.today || ""
                    opacity: 0.8
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            PlasmaComponents.Button {
                text: "Open Codex Minibar"
                icon.name: "codex-minibar"
                onClicked: root.callService("Toggle", [""])
            }

            PlasmaComponents.Button {
                icon.name: "view-refresh"
                text: "Refresh"
                enabled: root.running
                onClicked: {
                    root.callService("Refresh", []);
                    root.refreshNow();
                }
            }
        }
    }
}
