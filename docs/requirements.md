# Requirements

The product owner's requirements, as given on 26 October 2026. Decisions and details are in `CLAUDE.md`, and the spec will live in `docs/specs/`.

- [ ] A mascot that gives the app its name (**Binder**). Its brain is visible on its head, and the tone is lightly humorous.
- [ ] Knows the user in detail: identity and role, working style, technical taste, projects and goals.
- [ ] Knows the machines: this Mac, servers, the homelab.
- [ ] Knows hosting and app preferences.
- [ ] Knows SSH keys: which key opens which host, never the private material.
- [ ] Ships as a CLI and a real app.
- [ ] Marketing line: AG~~I~~M, Artificial General ~~Intelligence~~ Memory.
- [ ] Audience: individual developers and hobbyists first.
- [ ] Main problem: long sessions get worse, fresh sessions don't know the topic. Binder removes that trade-off.
- [ ] Everything in English: file names, app, CLI, site.
- [ ] First launch: the user fills in details about themselves and the above. Detect everything possible automatically; only personal details are typed in.
- [ ] Menu bar: mainly for approving potential context from the current session.
- [ ] Main window: detailed settings and everyday use.
- [ ] Design: several options were made in Claude Design (two red, one other colour). Direction C, Phosphor, was chosen.
- [ ] Handoff: a warning when a session gets long, and a manual handoff at any time.
