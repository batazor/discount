import Discount

/-!
Line-oriented reference executable: one `PricingInput` JSON document per input line, one
envelope per output line. A document that does not decode yields `{"decode_error": "..."}`.
-/
open Lean (Json)

def handleLine (line : String) : String :=
  let out : Json :=
    match Json.parse line >>= Lean.fromJson? (α := Discount.PricingInput) with
    | .error e => Json.mkObj [("decode_error", Json.str e)]
    | .ok i => Discount.envelope (Discount.apply i)
  out.compress

partial def loop (stdin : IO.FS.Stream) (stdout : IO.FS.Stream) : IO Unit := do
  let line ← stdin.getLine
  if line.isEmpty then return
  let t := line.trimAscii.toString
  if !t.isEmpty then
    stdout.putStrLn (handleLine t)
    stdout.flush
  loop stdin stdout

def main : IO Unit := do
  loop (← IO.getStdin) (← IO.getStdout)
