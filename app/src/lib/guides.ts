/** Each screen's guide, shown over the window the first time the screen opens and again with `?`: a picture of the
 *  screen, what it is for, what it shows and does, and what it needs. Closed ones are kept in `tips_hidden`. */
import agents from "../assets/guide/agents.jpg";
import cloud from "../assets/guide/cloud.jpg";
import devices from "../assets/guide/devices.jpg";
import github from "../assets/guide/github.jpg";
import overview from "../assets/guide/overview.jpg";
import printer from "../assets/guide/printer.jpg";
import projects from "../assets/guide/projects.jpg";
import server from "../assets/guide/server.jpg";
import sites from "../assets/guide/sites.jpg";
import tasks from "../assets/guide/tasks.jpg";
import vpn from "../assets/guide/vpn.jpg";

/** Something a screen needs, checked on this Mac or in the config. */
export type Need = "server" | "gh" | "tailscale" | "vault" | "cloud" | "printer";

export const NEEDS: Record<Need, string> = {
  server: "A home server you reach with SSH",
  gh: "GitHub's gh, signed in",
  tailscale: "Tailscale on this Mac",
  vault: "Your Obsidian vault folder",
  cloud: "The Cloud share mounted",
  printer: "A Bambu printer in LAN mode",
};

export interface Guide {
  title: string;
  /** One line on what the screen is for. */
  lead: string;
  /** Three short points on what it shows and does. */
  points: [string, string, string];
  needs: Need[];
  /** The screen with demo data, from scripts/guide-shots.sh. */
  shot: string;
}

export const GUIDES: Record<string, Guide> = {
  overview: {
    title: "Overview",
    lead: "Your day at a glance, in widgets you arrange.",
    points: ["What happened since you last looked: containers, prints, restarts and issues.", "Open tasks, today's done lines, the server, Cloud and the printer.", "Edit moves and sizes the widgets; the layout is kept."],
    needs: [],
    shot: overview,
  },
  tasks: {
    title: "Tasks",
    lead: "One board for you and the Claude Code agent on your server.",
    points: ["Press N for a new task. Dates, times, #project and !priority work in the title.", "Assign a task to the agent and it picks it up, or runs it at night.", "Board or list, with repeating tasks and reminders."],
    needs: [],
    shot: tasks,
  },
  projects: {
    title: "Projects",
    lead: "Your GitHub repos, each with a note in your Obsidian vault.",
    points: ["Sync brings repos in through gh: description, language, pull requests and CI.", "Done today lines land in the project's daily note.", "Each project has its tasks, its folder on this Mac and one for the agent."],
    needs: ["gh", "vault"],
    shot: projects,
  },
  github: {
    title: "GitHub",
    lead: "Pull requests and issues of your projects, answered from here.",
    points: ["Needs you, Mine and All open, across every project.", "Review, merge, comment and close through gh on this Mac.", "The agent can review a pull request or work on an issue on your server."],
    needs: ["gh"],
    shot: github,
  },
  devices: {
    title: "Devices",
    lead: "Your computers and who is online on your tailnet.",
    points: ["Each device checked over the network or through Tailscale.", "Ping, route and keys about to expire, with alerts.", "Send files with Taildrop and open an SSH shell."],
    needs: ["tailscale"],
    shot: devices,
  },
  vpn: {
    title: "VPN",
    lead: "Your own exit nodes as VPN regions.",
    points: ["One switch sends this Mac's traffic out through an exit node.", "Each node is placed on the globe by its region.", "Ping, live speed and totals while it is on."],
    needs: ["tailscale"],
    shot: vpn,
  },
  server: {
    title: "Servers",
    lead: "Your home server, read over SSH.",
    points: ["CPU, memory, disks, temperature and uptime.", "Containers and services, with logs and a restart you confirm.", "Alerts when a container stops or a disk fills."],
    needs: ["server"],
    shot: server,
  },
  sites: {
    title: "Sites",
    lead: "The websites on your server, watched.",
    points: ["Every compose project in ~/sites and every Cloudflare tunnel, found by itself.", "A still preview, the answer time and 7 days of uptime.", "CPU, memory and disk for each site."],
    needs: ["server"],
    shot: sites,
  },
  agents: {
    title: "Agents",
    lead: "Terminals into tmux sessions on your server.",
    points: ["Claude Code, Ollama and a session per project, ⌘1 to ⌘9.", "Each shows whether it is working, waiting for you or done.", "Sessions keep running when Binder closes."],
    needs: ["server"],
    shot: agents,
  },
  cloud: {
    title: "Cloud",
    lead: "A file manager for a folder on your server.",
    points: ["Upload, download, move and zip, with a transfer tray.", "Deletes go to a trash you can restore from.", "Big copies run on the server itself when SSH is set."],
    needs: ["cloud"],
    shot: cloud,
  },
  printer: {
    title: "3D Printer",
    lead: "A Bambu printer on your network.",
    points: ["Progress, temperatures and the camera, live.", "Health messages in Bambu's own words, and a notification when a print ends.", "The chamber light, the part fan and every past print."],
    needs: ["printer"],
    shot: printer,
  },
};

export const ALL_GUIDES = Object.keys(GUIDES);
