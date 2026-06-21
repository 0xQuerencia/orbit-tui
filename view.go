package main

// imports
import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

func (m *Model) View() string {
	if m.ScreenWidth == 0 {
		return "Starting Orbit..."
	}
	th := themes[m.ThemeIndex]

	body := renderActiveBody(m, th)
	if m.ShowingHelp {
		body = renderHelpPage(th)
	} else {
		body = padPanelBody(body, m.ScreenHeight-12)
	}

	topBar := headerStyle(th).
		Width(m.ScreenWidth).
		Align(lipgloss.Center).
		Render(fmt.Sprintf("◉ Orbit %s", m.Spinner.View()))

	tabRow := lipgloss.JoinHorizontal(lipgloss.Top, buildTabRowForModel(m, th)...)
	tabStrip := lipgloss.NewStyle().
		Width(m.ScreenWidth).
		Align(lipgloss.Center).
		Render(tabRow)

	content := cardStyle(th, m.ScreenWidth).Render(body)
	footer := renderFooterBar(m, th)

	return lipgloss.JoinVertical(lipgloss.Left,
		"",
		topBar,
		"",
		tabStrip,
		"",
		content,
		"",
		footer,
	)
}

func renderActiveBody(m *Model, th Theme) string {
	switch m.ActiveTab {
	case 0:
		return renderDashboardView(m, th)
	case 1:
		return renderProcessesView(m, th)
	case 2:
		return renderPortsView(m, th)
	case 3:
		return renderConnectionsView(m, th)
	}
	return renderDashboardView(m, th)
}

func renderFooterBar(m *Model, th Theme) string {
	dot := mutedStyle(th).Render("•")
	parts := []string{
		labelStyle(th).Render("q: quit"),
		dot,
		labelStyle(th).Render("tab/←/→: switch"),
		dot,
		labelStyle(th).Render("↑/↓: move"),
		dot,
		labelStyle(th).Render("enter: kill"),
		dot,
		labelStyle(th).Render("?: help"),
		dot,
		labelStyle(th).Render("shift+←/→: theme"),
	}

	line := strings.Join(parts, " ")
	return lipgloss.NewStyle().
		Width(m.ScreenWidth).
		Align(lipgloss.Center).
		Render(mutedStyle(th).Render(line))
}

func padPanelBody(body string, targetLines int) string {
	if targetLines < 0 {
		targetLines = 0
	}
	currentLines := 1
	if body != "" {
		currentLines += strings.Count(body, "\n")
	}
	if currentLines >= targetLines {
		return body
	}
	return body + strings.Repeat("\n", targetLines-currentLines)
}

func buildTabRowForModel(m *Model, th Theme) []string {
	tabs := make([]string, len(m.TabNames))
	for i, name := range m.TabNames {
		tabs[i] = tabStyle(th, i == m.ActiveTab).Render(name)
	}
	return tabs
}

func renderHelpPage(th Theme) string {
	return lipgloss.JoinVertical(lipgloss.Left,
		sectionStyle(th).Render("KEYBINDS"),
		"",
		labelStyle(th).Render("Tab Navigation:"),
		"  -> / Tab         Switch to next tab",
		"  <- / Shift+Tab   Switch to previous tab",
		"  Q / Ctrl+C      Quit Orbit",
		"",
		labelStyle(th).Render("Processes & Ports:"),
		"  Up / Down        Navigate rows",
		"  Enter            Kill selected process",
		"",
		labelStyle(th).Render("Display:"),
		"  Shift+Left/Right  Switch theme",
		"  ?                 Toggle help",
		"",
		mutedStyle(th).Render("Press ? in any tab to close help"),
	)
}

func renderDashboardView(m *Model, th Theme) string {
	lines := []string{
		sectionStyle(th).Render("— SYSTEM —"),
		warnLabelStyle(m.CPUWarn, "CPU", th) + " " +
			valueStyle(th).Render(fmt.Sprintf("%.0f%%", m.CPUUsage*100)),
		m.CPUProgress.ViewAs(m.CPUUsage),
		warnLabelStyle(m.MemWarn, "RAM", th) + " " +
			valueStyle(th).Render(fmt.Sprintf("%s/%s", m.MemUsed, m.MemTotal)),
		m.MemProgress.ViewAs(m.MemUsage),
		"  " + mutedStyle(th).Render("Available: "+m.MemAvail),
		"  " + mutedStyle(th).Render("Swap: "+fmt.Sprintf("%s/%s", m.SwapUsed, m.SwapTotal)),
		warnLabelStyle(m.DiskWarn, "Disk", th) + " " +
			valueStyle(th).Render(fmt.Sprintf("%s/%s", m.DiskUsed, m.DiskTotal)),
		m.DiskProgress.ViewAs(m.DiskUsage),
		valueStyle(th).Render("Up " + m.SystemUptime),
		"",
		sectionStyle(th).Render("— I/O —"),
		"  ↙ " + valueStyle(th).Render(m.DiskRead),
		"  ↗ " + valueStyle(th).Render(m.DiskWrite),
		"",
		sectionStyle(th).Render("— HARDWARE —"),
	}

	if m.GPUName != "" {
		lines = append(lines,
			"  GPU "+valueStyle(th).Render(fmt.Sprintf("%.0f%% %s", m.GPUUsage, m.GPUMemUsed)))
	} else {
		lines = append(lines, "  "+mutedStyle(th).Render("GPU (not available)"))
	}

	if m.BattPercent >= 0 {
		icon := "⚡"
		if m.BattState == "Charging" {
			icon = "⚡"
		} else if m.BattState == "Discharging" {
			icon = "⚡"
		}
		lines = append(lines,
			"  "+labelStyle(th).Render(icon)+" "+valueStyle(th).Render(fmt.Sprintf("%d%%", m.BattPercent)))
		if m.BattState != "" || m.BattTime != "" {
			lines = append(lines,
				"    "+mutedStyle(th).Render(strings.TrimSpace(m.BattState+" "+m.BattTime)))
		}
	}

	lines = append(lines,
		"",
		sectionStyle(th).Render("— NETWORK —"),
		"  ↑ "+valueStyle(th).Render(m.NetSent),
		"  ↓ "+valueStyle(th).Render(m.NetRecv),
		"  ◉ "+valueStyle(th).Render(fmt.Sprintf("%d ports", len(m.PortList))),
		"  🌐 "+valueStyle(th).Render(m.PublicIP),
		"",
	)

	if m.StatusMsg != "" {
		lines = append(lines, "", mutedStyle(th).Render("> "+m.StatusMsg))
	}

	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

func renderProcessesView(m *Model, th Theme) string {
	return m.ProcTable.View()
}

func renderPortsView(m *Model, th Theme) string {
	return m.PortTable.View()
}

func renderConnectionsView(m *Model, th Theme) string {
	return m.ConnTable.View()
}
