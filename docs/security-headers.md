# Security headers

The content security policy (CSP) of the project lives in one file, `csp.json` at the project root. Everything else is
derived from it or judged against it. This note says who sends the policy, what a `<meta>` cannot carry, how a project
moves from `report-only` to `enforce`, and what to put into the hosting config.

## What sends the policy

`csp.json` has `mode` (`enforce` or `report-only`), `directives` (a directive name → a non-empty list of sources, emitted
in its order) and an optional `reportUri` (an `https://` URL). Any other field is a `csp/config` violation. It is a seed:
`create` writes it, the project owns its origins, `update` never touches it.

| Who                         | What it sends                                                                                        |
| --------------------------- | ---------------------------------------------------------------------------------------------------- |
| the Vite plugin `kit-csp`   | in `enforce`, a `<meta http-equiv="Content-Security-Policy">` at the start of `head` of `vite build` |
| `vite preview`              | the header `Content-Security-Policy` (or `-Report-Only`) on every response                           |
| the hosting (`vercel.json`) | the same header in both modes: the only carrier of `frame-ancestors`, `report-uri` and Report-Only   |
| `pnpm csp:check`            | nothing: it judges `csp.json`, `index.html` and `vercel.json`                                        |

`vite dev` gets no policy: the React refresh preamble is an inline script and `script-src 'self'` would block it. Do not
write the meta into `index.html` by hand (`csp/hand-meta`): it would also reach `vite dev`, and every copy of
`index.html` would drift.

## What a meta cannot carry

A `<meta>` policy ignores `frame-ancestors`, `report-uri`, `report-to` and `sandbox` (W3C CSP3 §3.3; Chrome logs a
console warning for each), and it cannot be Report-Only. So the build meta is the header policy without those four, and
the rest lives in the hosting header:

- `frame-ancestors` (CSP3 §6.4.2) decides who may frame the app; the Telegram policy needs it for web.telegram.org.
- `report-uri` sends the violation reports to a collector.
- The Report-Only form blocks nothing and reports each violation, so it is the safe first step.

The build meta is a second line of defence for a host that sends no header; the header is the policy of record.

## The two modes

- `report-only`: no build meta; the hosting header and the preview send `Content-Security-Policy-Report-Only`. The browser
  blocks nothing, reports each violation (to `reportUri` when set) and fires `securitypolicyviolation`, so the e2e still
  catches one. A build whose environment names an origin outside `connect-src` only warns.
- `enforce`: the build meta and the header `Content-Security-Policy`, from the same directives. A build whose
  `VITE_API_URL`, `VITE_SENTRY_DSN`, `VITE_VITALS_URL` or `VITE_SUPABASE_URL` names an origin outside `connect-src`
  stops and names the variable and the origin.

**From `report-only` to `enforce`.** Move when a full release cycle (the build in production and the e2e `prod` project)
shows no violation report. The move is one edit of `mode` in `csp.json` and the header key in the hosting config, which
`pnpm csp:check` names (`Content-Security-Policy-Report-Only` → `Content-Security-Policy`). The web-react reference
starts in `enforce`; the Telegram policy starts in `report-only` until the device run inside Telegram clients. An adopted
project with its own header gets a `csp.json` made from that header, in its mode (`report-only` for the Report-Only
key), so `csp:check` is green right after the adopt; a header it cannot read gives the kit `csp.json` and a note; a project without `csp.json` has the check
off.

## The reference policy and the Telegram policy

The web-react reference (`csp.json` of the pack):

```
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

`data:` in `img-src` covers the small assets Vite inlines. React writes `style` props through the CSSOM, which
`style-src` does not govern, so `style-src 'self'` is enough. Nobody frames the app: `frame-ancestors 'none'`.

The Telegram pack overrides it in a swift project (`report-only`):

```
default-src 'self'; script-src 'self' https://telegram.org; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'self' https://web.telegram.org
```

| Source                                     | Why                                                                                                                                                                                                           |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `script-src https://telegram.org`          | `telegram-web-app.js` loads from telegram.org                                                                                                                                                                 |
| `style-src 'unsafe-inline'`                | `telegram-web-app.js` creates `<style>` elements and fills them through `innerHTML` (the bottom-button shine, the custom style web.telegram.org posts into a framed app); a hash or a nonce cannot cover them |
| `img-src https:`                           | the user photos that Telegram serves from its own hosts                                                                                                                                                       |
| `frame-ancestors https://web.telegram.org` | Telegram Web K and A frame the Mini App; the mobile and desktop clients use a WebView and no frame                                                                                                            |

`'unsafe-inline'` stays out of `script-src` and `default-src`: `csp/script-src` rejects it there.

## The header for the host

On Vercel the header is an entry of `vercel.json` for the source `/(.*)`, and `pnpm csp:check` compares it with
`csp.json`:

```json
{
  "headers": [
    {
      "source": "/(.*)",
      "headers": [{ "key": "Content-Security-Policy", "value": "<the header policy>" }]
    }
  ]
}
```

Exactly one CSP header in that entry, keyed `Content-Security-Policy` in `enforce` and `Content-Security-Policy-Report-Only`
in `report-only`. A mismatch prints the line to paste: `violation csp/hosting — vercel.json: … set "KEY: VALUE"`.

Other hosts take the same `KEY: VALUE` line:

```nginx
add_header Content-Security-Policy "<the header policy>" always;
```

```
# _headers (Netlify, Cloudflare Pages)
/*
  Content-Security-Policy: <the header policy>
```

`csp:check` reads only `vercel.json`; on another host keep the value equal to `toPolicy(config, 'header')` by hand.

## New origins

A new origin goes into `connect-src` of `csp.json` first: an API on another origin (`VITE_API_URL`), the Sentry ingest
host (`VITE_SENTRY_DSN`), the Web Vitals beacon (`VITE_VITALS_URL`) and Supabase (`VITE_SUPABASE_URL`). A build in
`enforce` stops otherwise. A source is `*`, a scheme (`https:`), a host with a scheme and an optional port, or a `*.`
host that matches any subdomain but not the apex. `'self'` never allows another origin. Then update the hosting header
to the new value (`csp/hosting`).

## Reports

`reportUri` is emitted as `report-uri` in the header only. Point it at a collector; for Sentry, use the security endpoint
of the project (Project settings → Security Headers) as the `https://` URL. The kit ships no collector.
