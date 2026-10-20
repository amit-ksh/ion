import { Redirect } from "expo-router";
/** The native browser session owns code/state validation. Never render or log
 * callback parameters; a standalone deep link grants no account access. */
export default function AuthCallback() {
  return <Redirect href="/daemons" />;
}
