import adobeIcon from "../assets/site-icons/adobe.svg";
import amazonIcon from "../assets/site-icons/amazon.svg";
import appleIcon from "../assets/site-icons/apple.svg";
import canvaIcon from "../assets/site-icons/canva.svg";
import discordIcon from "../assets/site-icons/discord.svg";
import dropboxIcon from "../assets/site-icons/dropbox.svg";
import facebookIcon from "../assets/site-icons/facebook.svg";
import githubIcon from "../assets/site-icons/github.svg";
import gitlabIcon from "../assets/site-icons/gitlab.svg";
import googleIcon from "../assets/site-icons/google.svg";
import instagramIcon from "../assets/site-icons/instagram.svg";
import linkedinIcon from "../assets/site-icons/linkedin.svg";
import microsoftIcon from "../assets/site-icons/microsoft.svg";
import netflixIcon from "../assets/site-icons/netflix.svg";
import openaiIcon from "../assets/site-icons/openai.svg";
import redditIcon from "../assets/site-icons/reddit.svg";
import spotifyIcon from "../assets/site-icons/spotify.svg";
import tiktokIcon from "../assets/site-icons/tiktok.svg";
import xIcon from "../assets/site-icons/x.svg";
import yahooIcon from "../assets/site-icons/yahoo.svg";

const serviceDefinitions = {
  google: {
    label: "Google",
    icon: googleIcon,
    domains: ["google.com", "gmail.com"],
  },
  microsoft: {
    label: "Microsoft",
    icon: microsoftIcon,
    domains: ["microsoft.com", "live.com", "outlook.com", "microsoftonline.com"],
  },
  facebook: { label: "Facebook", icon: facebookIcon, domains: ["facebook.com"] },
  instagram: { label: "Instagram", icon: instagramIcon, domains: ["instagram.com"] },
  x: { label: "X", icon: xIcon, domains: ["x.com", "twitter.com"] },
  github: { label: "GitHub", icon: githubIcon, domains: ["github.com"] },
  gitlab: { label: "GitLab", icon: gitlabIcon, domains: ["gitlab.com"] },
  linkedin: { label: "LinkedIn", icon: linkedinIcon, domains: ["linkedin.com"] },
  amazon: {
    label: "Amazon",
    icon: amazonIcon,
    domains: ["amazon.com", "amazon.ca", "amazon.co.uk", "amazon.de", "amazon.fr", "amazon.in", "amazon.co.jp"],
  },
  apple: { label: "Apple", icon: appleIcon, domains: ["apple.com", "icloud.com"] },
  discord: { label: "Discord", icon: discordIcon, domains: ["discord.com", "discord.gg"] },
  reddit: { label: "Reddit", icon: redditIcon, domains: ["reddit.com"] },
  yahoo: { label: "Yahoo", icon: yahooIcon, domains: ["yahoo.com"] },
  netflix: { label: "Netflix", icon: netflixIcon, domains: ["netflix.com"] },
  spotify: { label: "Spotify", icon: spotifyIcon, domains: ["spotify.com"] },
  dropbox: { label: "Dropbox", icon: dropboxIcon, domains: ["dropbox.com"] },
  canva: { label: "Canva", icon: canvaIcon, domains: ["canva.com"] },
  adobe: { label: "Adobe", icon: adobeIcon, domains: ["adobe.com"] },
  tiktok: { label: "TikTok", icon: tiktokIcon, domains: ["tiktok.com"] },
  openai: { label: "OpenAI", icon: openaiIcon, domains: ["openai.com", "chatgpt.com"] },
} as const;

export type ServiceKey = keyof typeof serviceDefinitions;
export type ServiceIdentity = (typeof serviceDefinitions)[ServiceKey];

function hostnameFromWebsite(website?: string | null) {
  const value = website?.trim();
  if (!value) return null;

  try {
    const url = new URL(value.includes("://") ? value : `https://${value}`);
    return url.hostname.toLocaleLowerCase().replace(/\.$/, "") || null;
  } catch {
    return null;
  }
}

export function getServiceKeyFromUrl(website?: string | null): ServiceKey | null {
  const hostname = hostnameFromWebsite(website);
  if (!hostname) return null;

  for (const [key, service] of Object.entries(serviceDefinitions)) {
    if (service.domains.some((domain) => hostname === domain || hostname.endsWith(`.${domain}`))) {
      return key as ServiceKey;
    }
  }

  return null;
}

export function getServiceIdentity(website?: string | null): ServiceIdentity | null {
  const key = getServiceKeyFromUrl(website);
  return key ? serviceDefinitions[key] : null;
}
