// One shape for every action result, so a page can render success, the API's verbatim
// error, the console's own field errors, or "service unreachable" the same way, and always
// repopulate the form with what the operator typed.
import { call, type Outcome } from "../api/client";
import { fieldsOf, type FieldError, type Fields, type Parsed } from "../model/fields";

export type ActionOutcome<T> =
  | { kind: "ok"; data: T; fields: Fields }
  | { kind: "api"; status: number; body: unknown; fields: Fields; traceId: string; step?: string; saved?: string[] }
  | { kind: "invalid"; errors: FieldError[]; fields: Fields }
  | { kind: "unreachable"; url: string; fields: Fields; traceId: string; step?: string; saved?: string[] };

type Reply<T> = { data?: T; error?: unknown; response: Response };

/** Parse the form with a view-model function, send the body, fold the outcome. */
export async function run<B, T>(
  fd: FormData,
  parse: (fields: Fields) => Parsed<B>,
  send: (body: B) => Promise<Reply<T>>,
): Promise<ActionOutcome<T>> {
  const fields = fieldsOf(fd);
  const parsed = parse(fields);
  if (!parsed.ok) return { kind: "invalid", errors: parsed.errors, fields };
  return fold(await call(() => send(parsed.value)), fields);
}

/** For actions that need no body parsing (delete, confirm, cancel). */
export async function runPlain<T>(fd: FormData, send: (fields: Fields) => Promise<Reply<T>>): Promise<ActionOutcome<T>> {
  const fields = fieldsOf(fd);
  return fold(await call(() => send(fields)), fields);
}

export function fold<T>(o: Outcome<T>, fields: Fields): ActionOutcome<T> {
  if (o.ok) return { kind: "ok", data: o.data, fields };
  if (o.unreachable) return { kind: "unreachable", url: o.url, fields, traceId: o.traceId };
  return { kind: "api", status: o.status, body: o.body, fields, traceId: o.traceId };
}

/** What a form needs to render after an action (or before any): fields, field errors, API error. */
export type FormState = {
  fields: Fields;
  errors: FieldError[];
  api: { status: number; body: unknown } | null;
  unreachable: string | null;
  /** Trace id of the failed call (api or unreachable), for the error panel. */
  traceId: string | null;
  ok: boolean;
  /** For multi-step actions: the step that failed and the steps saved before it. */
  step: string | null;
  saved: string[];
};

export function formState(result: ActionOutcome<unknown> | undefined, initial: Fields): FormState {
  if (!result) return { fields: initial, errors: [], api: null, unreachable: null, traceId: null, ok: false, step: null, saved: [] };
  return {
    // After a success the page re-reads the record, so the fresh fields win.
    fields: result.kind === "ok" ? initial : result.fields,
    errors: result.kind === "invalid" ? result.errors : [],
    api: result.kind === "api" ? { status: result.status, body: result.body } : null,
    unreachable: result.kind === "unreachable" ? result.url : null,
    traceId: result.kind === "api" || result.kind === "unreachable" ? result.traceId : null,
    ok: result.kind === "ok",
    step: result.kind === "api" || result.kind === "unreachable" ? (result.step ?? null) : null,
    saved: result.kind === "api" || result.kind === "unreachable" ? (result.saved ?? []) : [],
  };
}
