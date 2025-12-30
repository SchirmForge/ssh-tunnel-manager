// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    id: profilesPage
    title: "SSH Tunnel Profiles"

    // Static placeholder data (Qt model wiring will replace this)
    ListModel {
        id: profilesModel
        ListElement {
            idStr: "11111111-1111-1111-1111-111111111111"
            name: "Example Tunnel"
            host: "example.com"
            statusText: "Not connected"
            statusColor: "#9e9e9e"
            canStart: true
            canStop: false
        }
        ListElement {
            idStr: "22222222-2222-2222-2222-222222222222"
            name: "Database Tunnel"
            host: "db.internal"
            statusText: "Connected"
            statusColor: "#4caf50"
            canStart: false
            canStop: true
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent

            Label {
                text: "Profiles"
                font.bold: true
                font.pixelSize: 16
                Layout.fillWidth: true
                leftPadding: 12
            }

            ToolButton {
                text: "+"
                font.pixelSize: 20
                onClicked: {
                    // TODO: Open profile dialog
                    console.log("Add profile clicked")
                }
            }

            ToolButton {
                text: "⟳"
                font.pixelSize: 16
                onClicked: profilesModel.refresh()
            }
        }
    }

    // Main content
    ListView {
        id: profilesListView
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8
        clip: true

        model: profilesModel

        delegate: ItemDelegate {
            width: profilesListView.width - 16
            height: 80

            // Profile data from model (placeholder JSON for now)
            property var profileData: ({
                id: idStr,
                name: name,
                host: host,
                statusText: statusText,
                statusColor: statusColor,
                canStart: canStart,
                canStop: canStop
            })

            background: Rectangle {
                color: parent.hovered ? "#f0f0f0" : "white"
                border.color: "#cccccc"
                border.width: 1
                radius: 6
            }

            contentItem: RowLayout {
                spacing: 12

                // Status indicator (color from ProfileViewModel!)
                Rectangle {
                    width: 12
                    height: 12
                    radius: 6
                    color: profileData.statusColor
                    Layout.alignment: Qt.AlignVCenter
                }

                // Profile info
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: profileData.name
                        font.bold: true
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }

                    Label {
                        text: profileData.host
                        font.pixelSize: 11
                        color: "#666666"
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }

                    Label {
                        text: profileData.statusText
                        font.pixelSize: 11
                        color: "#444444"
                    }
                }

                // Action buttons
                RowLayout {
                    spacing: 4

                    Button {
                        text: "Start"
                        enabled: profileData.canStart
                        onClicked: {
                            console.log("Start tunnel:", profileData.id)
                            // TODO: Call daemon to start tunnel
                        }
                    }

                    Button {
                        text: "Stop"
                        enabled: profileData.canStop
                        onClicked: {
                            console.log("Stop tunnel:", profileData.id)
                            // TODO: Call daemon to stop tunnel
                        }
                    }

                    Button {
                        text: "Edit"
                        onClicked: {
                            console.log("Edit profile:", profileData.id)
                            // TODO: Open profile dialog
                        }
                    }
                }
            }

            onClicked: {
                console.log("Profile clicked:", profileData.name)
            }
        }

        // Empty state
        Label {
            visible: profilesListView.count === 0
            anchors.centerIn: parent
            text: "No profiles yet\n\nClick + to create your first SSH tunnel profile"
            horizontalAlignment: Text.AlignHCenter
            color: "#999999"
            font.pixelSize: 14
        }
    }
}
