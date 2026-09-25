//! What the source tree itself is held to, read once and checked together.
//!
//! Every check here reads the same crawl of the workspace, which [`source_tree`] makes
//! once for the whole run, and the ones that ask about the shape of the code read the
//! same parse of it rather than matching text.

mod lexed;
mod refused;
mod shape;
mod source_tree;
mod spelling;

mod a_latch_is_settled_once;
mod each_requirement_is_claimed_once;
mod how_long_a_file_may_be;
mod modest_terminal;
mod nothing_a_plugin_brings_gets_its_own_account;
mod nothing_here_runs_what_somebody_sent;
mod nothing_leaves_on_its_own;
mod nothing_reports_on_you;
mod nothing_resolves_a_second_host;
mod nothing_shapes_this_machines_traffic;
mod one_number_one_place;
mod one_spelling_on_the_wire;
mod plain_language;
mod surface_parity;
mod the_one_way_out;
mod what_a_check_can_see;
mod what_a_comment_may_be;
mod what_a_form_holds;
mod what_a_migration_may_reach;
mod what_a_self_update_may_reach;
mod what_a_source_file_may_not_say;
mod what_a_verb_disturbs;
mod what_provenance_is_waiting_to_attribute;
mod what_seeding_does_in_order;
mod what_the_build_forbids;
mod what_the_coverage_report_can_see;
mod what_the_gate_measures;
mod what_windows_is_waiting_on;
mod where_it_listens;
mod where_the_outside_world_is_reached;
mod withholding;
