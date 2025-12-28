// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.ssh_tunnel_manager 1.0

Page {
    id: profilesPage
    title: "SSH Tunnel Profiles"

    // Create the profiles model
    ProfilesListModel {
        id: profilesModel
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

        model: profilesModel.items

        delegate: ItemDelegate {
            width: profilesListView.width - 16
            height: 80

            // Profile data from model (comes from ProfileViewModel!)
            // Parse JSON string from model
            property var profileData: JSON.parse(modelData)

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
