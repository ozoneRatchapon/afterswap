//! PL rating sanity: consistency beats one lucky spike.

use afterswap_engine::rating::plackett_luce;

#[test]
fn consistent_winner_outranks_lucky_spike() {
    // Item 0: always ranks first by a hair. Item 1: one huge win, otherwise
    // last. Item 2: always middle. Mean payoff would crown item 1
    // (avg 33.4 vs 1.0) — PL must crown item 0.
    let payoffs = vec![
        vec![1.0, 1.0, 1.0, 1.0, 1.0],
        vec![-1.0, -1.0, 168.0, -1.0, -1.0],
        vec![0.5, 0.5, 0.5, 0.5, 0.5],
    ];
    let g = plackett_luce(&payoffs);
    assert!(g[0] > g[2] && g[2] > g[1], "strengths {g:?}");
}

#[test]
fn deterministic_across_runs() {
    let payoffs = vec![vec![3.0, 1.0], vec![2.0, 2.0], vec![1.0, 3.0]];
    let a = plackett_luce(&payoffs);
    let b = plackett_luce(&payoffs);
    assert_eq!(a, b);
}
