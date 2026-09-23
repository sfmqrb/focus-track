// Omarchy bar widget for focus-track. Copy to ~/.config/omarchy/bar/modules/focus.qml and add
// { "id": "focus", "type": "qml" } to bar.layout.right in ~/.config/omarchy/shell.json.
// If focus-track is not in /usr/bin (e.g. installed to ~/.local/bin), use its full path below.
import QtQuick
import Quickshell.Io

// Focus ring: today's apps as arcs of one small ring (bigger arc = more time),
// with a dot in the middle that breathes while you're being tracked.
// Data comes from `focus-track widget`; no numbers on purpose.
Item {
  id: root
  property var bar
  property string moduleName
  property var settings

  property var slices: []
  property bool tracking: true
  property bool present: false
  property color accent: fg
  property string tip: "focus"

  readonly property color fg: bar ? bar.foreground : "white"
  readonly property int size: 15

  implicitWidth: size + 10
  implicitHeight: bar ? bar.barSize : 26

  Process {
    id: proc
    command: ["focus-track", "widget"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var d = JSON.parse(text)
          root.slices = d.slices
          root.tracking = d.tracking
          root.present = d.present
          root.accent = d.accent
          root.tip = d.tooltip
          ring.requestPaint()
        } catch (e) {}
      }
    }
  }

  Timer {
    interval: 30000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: if (!proc.running) proc.running = true
  }

  Canvas {
    id: ring
    anchors.centerIn: parent
    width: root.size
    height: root.size
    antialiasing: true

    onPaint: {
      var ctx = getContext("2d")
      ctx.reset()
      var c = width / 2, r = c - 1.5, gap = root.slices.length > 1 ? 0.35 : 0
      ctx.lineWidth = 2.2
      ctx.lineCap = "round"

      // faint full track
      ctx.globalAlpha = 0.18
      ctx.strokeStyle = root.fg
      ctx.beginPath()
      ctx.arc(c, c, r, 0, 2 * Math.PI)
      ctx.stroke()

      // one arc per app, starting at 12 o'clock
      ctx.globalAlpha = root.tracking ? 1 : 0.35
      var a = -Math.PI / 2
      for (var i = 0; i < root.slices.length; i++) {
        var sweep = root.slices[i][0] * 2 * Math.PI
        if (sweep > gap + 0.05) {
          ctx.strokeStyle = root.slices[i][1] || root.fg
          ctx.beginPath()
          ctx.arc(c, c, r, a + gap / 2, a + sweep - gap / 2)
          ctx.stroke()
        }
        a += sweep
      }
    }
  }

  // center dot: accent + breathing while you're here, dim when away, red if the daemon stopped
  Rectangle {
    anchors.centerIn: ring
    width: 5
    height: 5
    radius: 2.5
    color: !root.tracking ? (root.bar && root.bar.urgent ? root.bar.urgent : "#f7768e") : root.present ? root.accent : root.fg
    opacity: root.present || !root.tracking ? 1 : 0.3

    SequentialAnimation on scale {
      running: root.present
      loops: Animation.Infinite
      NumberAnimation { to: 0.55; duration: 1600; easing.type: Easing.InOutSine }
      NumberAnimation { to: 1.0; duration: 1600; easing.type: Easing.InOutSine }
    }
  }

  onFgChanged: ring.requestPaint()

  HoverHandler {
    onHoveredChanged: {
      if (!root.bar) return
      if (hovered) root.bar.showTooltip(root, root.tip)
      else root.bar.hideTooltip(root)
    }
  }

  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: if (root.bar) root.bar.run("omarchy-launch-or-focus-tui focus-track dashboard")
  }
}
