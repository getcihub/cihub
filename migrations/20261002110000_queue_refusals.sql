-- Lot 8. The reasons of a submission that the queue refuses while it drains or
-- switches its mode (AGENTS.md D151), in the order of the domain.
ALTER TYPE reason ADD VALUE 'queue_draining' AFTER 'mode_switch_completed';
ALTER TYPE reason ADD VALUE 'queue_switching_modes' AFTER 'queue_draining';
