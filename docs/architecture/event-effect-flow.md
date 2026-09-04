# Event and effect flow

`Input -> Action -> AppState transition -> Effect -> service -> Event -> AppState -> View -> Frame`

The root state is mutated only on the event-loop path. Workers receive immutable request data and
cannot access UI state. Replaceable work carries request/cancellation identity so stale results can be
discarded before changing visible state.

