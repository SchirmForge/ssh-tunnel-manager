// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
// Temporarily disabled until cxx-qt bridge issues resolved
// import com.ssh_tunnel_manager 1.0

ApplicationWindow {
    id: mainWindow
    visible: true
    width: 1000
    height: 700
    title: "SSH Tunnel Manager"

    // Navigation drawer
    Drawer {
        id: drawer
        width: 250
        height: parent.height

        ListView {
            anchors.fill: parent

            model: ListModel {
                ListElement { title: "Profiles"; page: "profiles" }
                ListElement { title: "Daemon Config"; page: "daemon" }
                ListElement { title: "About"; page: "about" }
            }

            delegate: ItemDelegate {
                text: model.title
                width: parent.width
                highlighted: stackView.currentItem.objectName === model.page
                onClicked: {
                    if (model.page === "profiles") {
                        stackView.replace(profilesPage)
                    } else if (model.page === "daemon") {
                        stackView.replace(placeholderPage, {pageName: "Daemon Configuration"})
                    } else if (model.page === "about") {
                        stackView.replace(aboutPage)
                    }
                    drawer.close()
                }
            }
        }
    }

    // Header
    header: ToolBar {
        RowLayout {
            anchors.fill: parent

            ToolButton {
                text: "☰"
                font.pixelSize: 20
                onClicked: drawer.open()
            }

            Label {
                text: mainWindow.title
                font.bold: true
                font.pixelSize: 16
                Layout.fillWidth: true
            }
        }
    }

    // Main content area
    StackView {
        id: stackView
        anchors.fill: parent
        initialItem: profilesPage
    }

    // Pages
    Component {
        id: profilesPage
        ProfilesList {
            objectName: "profiles"
        }
    }

    Component {
        id: aboutPage
        Page {
            objectName: "about"

            ColumnLayout {
                anchors.centerIn: parent
                spacing: 20

                Label {
                    text: "SSH Tunnel Manager"
                    font.pixelSize: 24
                    font.bold: true
                    Layout.alignment: Qt.AlignHCenter
                }

                Label {
                    text: "Version: 0.1.8"
                    font.pixelSize: 14
                    Layout.alignment: Qt.AlignHCenter
                }

                Rectangle {
                    width: 400
                    height: 2
                    color: "#cccccc"
                    Layout.alignment: Qt.AlignHCenter
                }

                Label {
                    text: "Qt6/QML GUI using cxx-qt"
                    font.pixelSize: 12
                    Layout.alignment: Qt.AlignHCenter
                }

                Label {
                    text: "~60-70% code reuse from gui-core"
                    font.pixelSize: 12
                    color: "#666666"
                    Layout.alignment: Qt.AlignHCenter
                }

                Rectangle {
                    width: 400
                    height: 2
                    color: "#cccccc"
                    Layout.alignment: Qt.AlignHCenter
                }

                Label {
                    text: "Apache-2.0 License"
                    font.pixelSize: 11
                    color: "#999999"
                    Layout.alignment: Qt.AlignHCenter
                }

                Label {
                    text: "© 2025 SSH Tunnel Manager Contributors"
                    font.pixelSize: 11
                    color: "#999999"
                    Layout.alignment: Qt.AlignHCenter
                }
            }
        }
    }

    Component {
        id: placeholderPage
        Page {
            property string pageName: ""
            objectName: "placeholder"

            Label {
                anchors.centerIn: parent
                text: pageName + "\n\nComing soon..."
                horizontalAlignment: Text.AlignHCenter
                font.pixelSize: 16
                color: "#999999"
            }
        }
    }
}
