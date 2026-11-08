import QtQuick
import QtQuick.Controls as QQC2

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    property alias cfg_executable: executableField.text
    property alias cfg_intervalSeconds: intervalField.value

    Kirigami.FormLayout {
        QQC2.TextField {
            id: executableField
            Kirigami.FormData.label: "Fallback Minibar executable:"
            placeholderText: "codex-minibar"
        }

        QQC2.SpinBox {
            id: intervalField
            Kirigami.FormData.label: "Update every (seconds):"
            from: 5
            to: 3600
        }
    }
}
