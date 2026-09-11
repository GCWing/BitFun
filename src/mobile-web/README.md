# Mobile web remote control

Open a desktop invitation and sign in with the desktop's GitHub account. The
browser remembers this login: refreshing, opening another tab with the same
invitation, or reopening the browser does not require another GitHub login while
the Relay token remains valid.

## Account and connection scope

- Login and the encrypted-RPC device identity belong to the browser profile,
  website origin, and Relay endpoint. Tabs share them through IndexedDB. This
  includes same-network HTTP invitations, where secure-context Web Locks are
  unavailable. Authentication uses a Bearer token, not an authentication cookie.
- Each tab independently selects its desktop and conversation. **Disconnect**
  returns that tab to device selection, including after reload. **Sign out** in
  Devices signs out the browser's tabs for that endpoint. Other browser profiles
  and the desktop account remain signed in.
- The desktop's Connected devices list receives the stable controller device ID
  as `ping.client.id`. Refreshes and new tabs therefore share one presence entry
  on a given desktop. Display names such as Chrome / macOS are labels, not keys;
  two real browser profiles with identical labels remain separate.
- A LAN address and the official site are different origins and have separate
  logins. Private browsing and clearing site data also create a separate or new
  identity. The Relay currently issues 30-day tokens; an actual HTTP 401 returns
  the affected tabs to sign-in. Network failures and offline desktops retain the
  saved account.

## Upgrade behavior

The first upgraded tab can migrate its v2 sessionStorage login together with the
original private key and controller ID. Other upgraded tabs adopt the shared
identity and migrate matching navigation. Sign-out leaves a revision marker so
a suspended legacy tab cannot restore an already signed-out token. Unknown or
unreadable records are preserved and shown as a storage error.

Tabs still running an older bundle may continue reporting their old presence
IDs until refreshed or closed. Those entries expire under the host's existing
75-second idle lease after their last ping; this client does not delete devices
or merge different profiles by their display name.

## Verification

`pnpm --dir src/mobile-web run test:account-browser` runs the actual React client
in independent Chrome tabs and persistent/disposable profiles. Requests are
intercepted for synthetic LAN HTTP and official HTTPS origins; the fixture
implements Relay authentication and encrypted host RPC. It checks shared login,
stable presence IDs, concurrent logins, delayed responses, cross-tab sign-out,
restart, legacy migration, expiry, and storage failures. Install Chrome/Chromium
or set `PUPPETEER_EXECUTABLE_PATH`. No live GitHub authorization occurs.

For live-host acceptance, use a desktop build serving this mobile bundle (and a
deployed bundle for the official site). Sign in once, open the same invitation
in a fresh tab without an opener, reload both tabs, and reopen the browser.
Confirm that the desktop retains one Connected devices entry. Select different
desktops in the two tabs, disconnect one tab, then sign out from Devices and
confirm both tabs return to sign-in. A separate browser profile should remain
independent. Repeat on LAN HTTP and official HTTPS; fixture tests alone are not
evidence of a live Relay/desktop deployment.
