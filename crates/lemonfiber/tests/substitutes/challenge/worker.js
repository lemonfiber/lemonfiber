const CLEARANCE = "lf_clearance";

const RELEASE = {
  title: "Big.Buck.Bunny.2008.1080p.BluRay.x264-LEMONFIBER",
  category: "2000",
  size: 1073741824,
  published: "Sun, 15 Nov 2026 00:00:00 +0000",
};

const encoder = new TextEncoder();

async function clearance(secret, agent) {
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signed = await crypto.subtle.sign("HMAC", key, encoder.encode(agent));
  return Array.from(new Uint8Array(signed), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function presented(request, name) {
  const header = request.headers.get("Cookie") ?? "";
  for (const pair of header.split(";")) {
    const [key, ...value] = pair.trim().split("=");
    if (key === name) {
      return value.join("=");
    }
  }
  return null;
}

function challenge(token) {
  const page =
    "<!DOCTYPE html><html><head><title>Just a moment...</title></head><body><script>" +
    `document.cookie = "${CLEARANCE}=${token}; path=/; Secure; SameSite=Lax"; location.reload();` +
    "</script></body></html>";
  return new Response(page, {
    status: 503,
    headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" },
  });
}

function xml(body) {
  return new Response(`<?xml version="1.0" encoding="UTF-8"?>${body}`, {
    headers: { "Content-Type": "application/xml; charset=utf-8", "Cache-Control": "no-store" },
  });
}

function caps() {
  return xml(
    '<caps><server version="1.0" title="lemonfiber challenge site"/>' +
      '<limits max="100" default="100"/>' +
      '<searching><search available="yes" supportedParams="q"/>' +
      '<tv-search available="no"/><movie-search available="no"/></searching>' +
      `<categories><category id="${RELEASE.category}" name="Movies"/></categories></caps>`,
  );
}

function found(origin) {
  const nzb = `${origin}/getnzb/1.nzb`;
  return xml(
    '<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">' +
      "<channel><title>lemonfiber challenge site</title><item>" +
      `<title>${RELEASE.title}</title><guid isPermaLink="false">1</guid><link>${nzb}</link>` +
      `<pubDate>${RELEASE.published}</pubDate>` +
      `<enclosure url="${nzb}" length="${RELEASE.size}" type="application/x-nzb"/>` +
      `<newznab:attr name="category" value="${RELEASE.category}"/>` +
      `<newznab:attr name="size" value="${RELEASE.size}"/>` +
      "</item></channel></rss>",
  );
}

export default {
  async fetch(request, env) {
    const token = await clearance(env.CLEARANCE_SECRET, request.headers.get("User-Agent") ?? "");
    if (presented(request, CLEARANCE) !== token) {
      return challenge(token);
    }
    const url = new URL(request.url);
    if (url.searchParams.get("t") === "caps") {
      return caps();
    }
    return found(url.origin);
  },
};
