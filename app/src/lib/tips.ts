/** A short guide at the top of each screen the first time it opens: what it is for and what it needs. Closed ones
 *  are kept in `tips_hidden`. */
export interface Tip {
  title: string;
  body: string;
}

export const TIPS: Record<string, Tip> = {
  overview: { title: "Overview", body: "Your day at a glance: open tasks, the server, Cloud and the printer. Edit moves and sizes the widgets." },
  tasks: { title: "Tasks", body: "A board for you and for the Claude Code agent on your server. Press N for a new task; dates like \"fri 9am\" and \"#project !high\" work in the title." },
  projects: { title: "Projects", body: "Your GitHub repos with a note each in your Obsidian vault. Sync brings repos in through gh; Done today lines land in the project's daily note." },
  github: { title: "GitHub", body: "Pull requests and issues of your projects, through gh on this Mac. The agent can review a pull request or work on an issue on your server." },
  devices: { title: "Devices", body: "Your computers, checked over the network or through Tailscale. Send files with Taildrop and open SSH from here." },
  vpn: { title: "VPN", body: "Each Tailscale exit node you run is a region. Turn one on to send this Mac's traffic out through it." },
  server: { title: "Servers", body: "CPU, memory, disks, containers and services of your home server, read over SSH. Binder only reads, except a container restart you confirm." },
  sites: { title: "Sites", body: "Websites on your server: folders in ~/sites with a compose file, and compose projects with a cloudflared container. Each is checked every 5 minutes." },
  agents: { title: "Agents", body: "Terminals into tmux sessions on your server: Claude Code, Ollama and your own. They keep running when Binder closes." },
  cloud: { title: "Cloud", body: "A file manager for a folder on your server shared over SMB. Deletes go to its trash; big copies run on the server when SSH is set." },
  printer: { title: "3D Printer", body: "A Bambu printer on your network over its local MQTT: progress, temperatures, the camera, and alerts in Bambu's own words." },
};

export const ALL_TIPS = Object.keys(TIPS);
