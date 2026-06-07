import { useEffect, useRef } from "react";
import {
  AnimatedSprite,
  Application,
  Assets,
  Container,
  Graphics,
  Rectangle,
  Sprite,
  Text,
  Texture,
  TilingSprite,
} from "pixi.js";

export type TraderView = { cohort: number; wealth: number; position: number; pnl: number };
export type CohortView = { name: string; color: string; herding: number };

type Data = { traders: TraderView[]; cohorts: CohortView[]; price: number; day: number; change: number };

const PACK = "/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets";
const CHAR_IDLE = `${PACK}/Characters/Human/IDLE/base_idle_strip9.png`;
const TREE = `${PACK}/Elements/Plants/spr_deco_tree_01_strip4.png`;
const expr = (n: string) => `${PACK}/UI/expression_${n}.png`;

const TS = 3; // tile display scale (16 -> 48)
const CF = { off: 28, top: 12, w: 40, h: 44, count: 9, stride: 96 };
const PLOT = 360; // horizontal spacing between guild plots
const MAP_H = 1000;

function sliceChar(tex: Texture): Texture[] {
  tex.source.scaleMode = "nearest";
  const out: Texture[] = [];
  for (let i = 0; i < CF.count; i++) out.push(new Texture({ source: tex.source, frame: new Rectangle(i * CF.stride + CF.off, CF.top, CF.w, CF.h) }));
  return out;
}
function sliceTree(tex: Texture): Texture[] {
  tex.source.scaleMode = "nearest";
  const out: Texture[] = [];
  for (let i = 0; i < 4; i++) out.push(new Texture({ source: tex.source, frame: new Rectangle(i * 32, 0, 32, 34) }));
  return out;
}

type Trader = { box: Container; pad: Graphics; sprite: AnimatedSprite; bubble: Sprite; hx: number; hy: number; h: number; lastLong: number; lastBubble: string };

type Props = Data & { onSelect?: (i: number | null) => void; selected?: number | null };

export default function SunnysideWorld(props: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const dataRef = useRef<Data>(props);
  dataRef.current = props;
  const onSelectRef = useRef(props.onSelect);
  onSelectRef.current = props.onSelect;
  const selectedRef = useRef<number | null | undefined>(props.selected);
  selectedRef.current = props.selected;

  useEffect(() => {
    let destroyed = false;
    let ro: ResizeObserver | null = null;
    const host = hostRef.current!;
    const app = new Application();

    const world = new Container();
    const ground = new Container();
    const statics = new Container(); // cottages, trees, tags, board
    const agents = new Container();
    world.addChild(ground, statics, agents);

    const pool: Trader[] = [];
    let charFrames: Texture[] = [];
    let treeFrames: Texture[] = [];
    let grassTex: Texture | null = null;
    let dirtTex: Texture | null = null;
    let waterTex: Texture | null = null;
    const moods: Record<string, Texture> = {};
    let boardText: Text | null = null;
    let mapW = 1400;
    let roadY = MAP_H * 0.54;
    let sigPrev = "";
    let lastDay = -1;
    let selRing: Graphics | null = null;

    const clampPan = () => {
      const a = appRef.current!;
      world.x = Math.min(0, Math.max(a.screen.width - mapW, world.x));
      world.y = Math.min(0, Math.max(a.screen.height - MAP_H, world.y));
    };
    const centerOn = (wx: number, wy: number) => {
      const a = appRef.current!;
      world.x = a.screen.width / 2 - wx;
      world.y = a.screen.height / 2 - wy;
      clampPan();
    };

    const plotX = (i: number, n: number) => (mapW - (n - 1) * PLOT) / 2 + i * PLOT;

    const rebuild = () => {
      if (!grassTex || !dirtTex || !waterTex) return;
      const cohorts = dataRef.current.cohorts;
      const n = Math.max(1, cohorts.length);
      mapW = Math.max(1300, n * PLOT + 280);
      roadY = Math.round(MAP_H * 0.54);

      ground.removeChildren();
      const grass = new TilingSprite({ texture: grassTex, width: mapW, height: MAP_H });
      grass.tileScale.set(TS);
      ground.addChild(grass);
      const road = new TilingSprite({ texture: dirtTex, width: mapW, height: 84 });
      road.tileScale.set(TS);
      road.position.set(0, roadY);
      ground.addChild(road);
      const plaza = new TilingSprite({ texture: dirtTex, width: 200, height: 150 });
      plaza.tileScale.set(TS);
      plaza.position.set(mapW / 2 - 100, roadY - 34);
      ground.addChild(plaza);
      const pond = new TilingSprite({ texture: waterTex, width: 230, height: 150 });
      pond.tileScale.set(TS);
      pond.position.set(40, MAP_H - 188);
      ground.addChild(pond);

      statics.removeChildren();
      statics.sortableChildren = true;
      // trees along the top and bottom edges
      let s = 5; const rnd = () => ((s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
      for (let i = 0; i < 22; i++) {
        if (!treeFrames.length) break;
        const t = new AnimatedSprite(treeFrames);
        t.animationSpeed = 0.04; t.play();
        t.anchor.set(0.5, 0.95);
        t.scale.set(TS * 0.85);
        const top = rnd() < 0.5;
        t.position.set(40 + rnd() * (mapW - 80), top ? 30 + rnd() * 60 : MAP_H - 30 - rnd() * 60);
        t.zIndex = t.y;
        statics.addChild(t);
      }
      // cottages + name tags per guild
      cohorts.forEach((c, i) => {
        const cx = plotX(i, n);
        const cottage = drawCottage(hex(c.color));
        cottage.position.set(cx, roadY - 6);
        cottage.zIndex = roadY - 6;
        statics.addChild(cottage);
        const tag = new Container();
        const g = new Graphics();
        g.roundRect(-52, -15, 104, 24, 7).fill({ color: hex(c.color) });
        const t = new Text({ text: c.name, style: { fill: "#ffffff", fontSize: 14, fontFamily: "system-ui", fontWeight: "700" } });
        t.anchor.set(0.5); t.position.set(0, -3);
        tag.addChild(g, t);
        tag.position.set(cx, roadY - 132);
        tag.zIndex = 1e6;
        statics.addChild(tag);
      });
      // notice board at plaza
      const board = new Container();
      const bg = new Graphics();
      bg.roundRect(-54, -36, 108, 44, 8).fill({ color: 0xffffff }).stroke({ color: 0x8b5a2b, width: 4 });
      bg.rect(-4, 8, 8, 20).fill({ color: 0x8b5a2b });
      boardText = new Text({ text: "", style: { fill: "#0f172a", fontSize: 18, fontFamily: "system-ui", fontWeight: "700" } });
      boardText.anchor.set(0.5); boardText.position.set(0, -14);
      board.addChild(bg, boardText);
      board.position.set(mapW / 2, roadY + 56);
      board.zIndex = 1e6;
      statics.addChild(board);

      // home slots for traders, grouped in front of each cottage
      ensurePool(dataRef.current.traders.length);
      const counts = new Array(n).fill(0);
      for (const tr of dataRef.current.traders) counts[tr.cohort]++;
      const cols = counts.map((c) => Math.max(1, Math.ceil(Math.sqrt(c))));
      const seen = new Array(n).fill(0);
      const cell = 34;
      for (let i = 0; i < dataRef.current.traders.length && i < pool.length; i++) {
        const ci = Math.min(dataRef.current.traders[i].cohort, n - 1);
        const slot = seen[ci]++;
        const c = cols[ci];
        const col = slot % c, row = Math.floor(slot / c);
        const gw = (c - 1) * cell;
        pool[i].hx = plotX(ci, n) - gw / 2 + col * cell;
        pool[i].hy = roadY + 110 + row * cell;
      }
    };

    const ensurePool = (count: number) => {
      while (pool.length < count) {
        const box = new Container();
        const pad = new Graphics();
        const sprite = new AnimatedSprite(charFrames);
        sprite.anchor.set(0.5, 0.92);
        sprite.animationSpeed = 0.08;
        sprite.currentFrame = Math.floor(Math.random() * charFrames.length);
        sprite.play();
        const bubble = new Sprite();
        bubble.anchor.set(0.5, 1); bubble.visible = false;
        box.addChild(pad, sprite, bubble);
        const myIndex = pool.length;
        box.eventMode = "static";
        box.cursor = "pointer";
        box.hitArea = new Rectangle(-16, -52, 32, 56);
        box.on("pointerdown", (e) => e.stopPropagation()); // don't start a map drag
        box.on("pointertap", (e) => { e.stopPropagation(); onSelectRef.current?.(myIndex); });
        agents.addChild(box);
        pool.push({ box, pad, sprite, bubble, hx: mapW / 2, hy: roadY + 110, h: 44, lastLong: 2, lastBubble: "" });
      }
      while (pool.length > count) pool.pop()!.box.destroy({ children: true });
    };

    const updateDay = () => {
      const { traders, cohorts, price, change } = dataRef.current;
      if (boardText) {
        boardText.text = price > 0 ? `$${price.toFixed(2)}` : "—";
        boardText.style.fill = change > 0 ? "#15803d" : change < 0 ? "#b91c1c" : "#0f172a";
      }
      let wMin = Infinity, wMax = -Infinity;
      for (const t of traders) { wMin = Math.min(wMin, t.wealth); wMax = Math.max(wMax, t.wealth); }
      const wSpan = Math.max(1e-6, wMax - wMin);
      for (let i = 0; i < traders.length && i < pool.length; i++) {
        const d = traders[i], tr = pool[i];
        tr.h = 40 + 26 * ((d.wealth - wMin) / wSpan);
        const long = d.position >= 0 ? 1 : 0;
        if (long !== tr.lastLong) {
          tr.lastLong = long;
          tr.pad.clear();
          tr.pad.ellipse(0, 2, 17, 7).fill({ color: long ? 0x22c55e : 0xef4444, alpha: 0.5 });
        }
        const b = bubbleFor(d.pnl, cohorts[d.cohort]?.herding ?? 0);
        if (b !== tr.lastBubble) {
          tr.lastBubble = b;
          if (b && moods[b]) {
            tr.bubble.texture = moods[b]; tr.bubble.visible = true;
            tr.bubble.height = 16; tr.bubble.width = 16 * (moods[b].width / Math.max(1, moods[b].height));
          } else tr.bubble.visible = false;
        }
        tr.box.zIndex = tr.hy;
      }
      agents.sortChildren();
    };

    const tick = () => {
      if (!appRef.current || charFrames.length === 0) return;
      const data = dataRef.current;
      const sig = data.cohorts.map((c) => c.name + c.color).join("|") + "#" + data.traders.length;
      if (sig !== sigPrev) { sigPrev = sig; rebuild(); centerOn(mapW / 2, roadY); lastDay = -1; }
      if (data.day !== lastDay) { lastDay = data.day; updateDay(); }
      const t = performance.now() / 1000;
      for (let i = 0; i < data.traders.length && i < pool.length; i++) {
        const tr = pool[i];
        tr.box.x = tr.hx + Math.sin(t * 0.5 + i * 1.7) * 6;
        tr.box.y = tr.hy + Math.cos(t * 0.42 + i * 2.3) * 4;
        const face = data.traders[i].position >= 0 ? 1 : -1;
        const sc = tr.h / CF.h;
        tr.sprite.scale.set(sc * face, sc);
        tr.bubble.position.set(0, -tr.h - 2);
      }
      // selection ring
      const sel = selectedRef.current;
      if (selRing) {
        if (sel != null && sel >= 0 && sel < pool.length && sel < data.traders.length) {
          const tr = pool[sel];
          selRing.clear();
          selRing.circle(0, -tr.h * 0.32, tr.h * 0.5).stroke({ color: 0x2563eb, width: 3, alpha: 0.95 });
          selRing.position.set(tr.box.x, tr.box.y);
          selRing.visible = true;
        } else {
          selRing.visible = false;
        }
      }
    };

    app
      .init({ background: 0x6cc24a, antialias: false, autoDensity: true, resolution: window.devicePixelRatio || 1, width: host.clientWidth || 480, height: host.clientHeight || 360 })
      .then(async () => {
        if (destroyed) { app.destroy(true, { children: true }); return; }
        appRef.current = app;
        app.ticker.maxFPS = 30;
        host.appendChild(app.canvas);
        agents.sortableChildren = true;
        app.stage.addChild(world);

        const [charT, treeT, grassT, dirtT, waterT, mLove, mWork, mStress, mAlert, mChat] = await Promise.all([
          Assets.load(CHAR_IDLE) as Promise<Texture>,
          Assets.load(TREE) as Promise<Texture>,
          Assets.load("/world/grass.png") as Promise<Texture>,
          Assets.load("/world/dirt.png") as Promise<Texture>,
          Assets.load("/world/water.png") as Promise<Texture>,
          Assets.load(expr("love")) as Promise<Texture>,
          Assets.load(expr("working")) as Promise<Texture>,
          Assets.load(expr("stress")) as Promise<Texture>,
          Assets.load(expr("alerted")) as Promise<Texture>,
          Assets.load(expr("chat")) as Promise<Texture>,
        ]);
        if (destroyed) return;
        charFrames = sliceChar(charT);
        treeFrames = sliceTree(treeT);
        grassTex = grassT; dirtTex = dirtT; waterTex = waterT;
        [grassT, dirtT, waterT].forEach((t) => (t.source.scaleMode = "nearest"));
        moods.love = mLove; moods.working = mWork; moods.stress = mStress; moods.alerted = mAlert; moods.chat = mChat;

        app.stage.eventMode = "static";
        app.stage.hitArea = app.screen;
        let drag = false, sx = 0, sy = 0, ox = 0, oy = 0;
        app.stage.on("pointerdown", (e) => { drag = true; sx = e.global.x; sy = e.global.y; ox = world.x; oy = world.y; });
        app.stage.on("pointermove", (e) => { if (!drag) return; world.x = ox + (e.global.x - sx); world.y = oy + (e.global.y - sy); clampPan(); });
        const stop = () => (drag = false);
        app.stage.on("pointerup", stop);
        app.stage.on("pointerupoutside", stop);
        app.stage.on("pointertap", () => onSelectRef.current?.(null)); // tap empty ground = deselect

        selRing = new Graphics();
        selRing.visible = false;
        world.addChild(selRing);

        const resize = () => { app.renderer.resize(host.clientWidth, host.clientHeight); clampPan(); };
        resize();
        ro = new ResizeObserver(resize); ro.observe(host);
        app.ticker.add(tick);
      });

    return () => {
      destroyed = true;
      ro?.disconnect();
      const a = appRef.current;
      if (a) { a.ticker.remove(tick); a.destroy(true, { children: true }); }
      appRef.current = null;
    };
  }, []);

  return <div ref={hostRef} className="h-full w-full cursor-grab overflow-hidden rounded-lg active:cursor-grabbing" />;
}

// A small flat-pixel cottage drawn from shapes (reliable, guild-coloured).
function drawCottage(roof: number): Graphics {
  const g = new Graphics();
  g.roundRect(-30, -42, 60, 46, 3).fill({ color: 0xe7d3ab }).stroke({ color: 0x7a5230, width: 2 });
  g.moveTo(-38, -38).lineTo(0, -70).lineTo(38, -38).closePath().fill({ color: roof }).stroke({ color: 0x5c3a1e, width: 2 });
  g.roundRect(-9, -22, 18, 26, 2).fill({ color: 0x6b4423 });
  g.rect(-23, -32, 12, 12).fill({ color: 0xbfe3ff });
  g.rect(11, -32, 12, 12).fill({ color: 0xbfe3ff });
  return g;
}

function bubbleFor(pnl: number, herding: number): string {
  if (pnl > 0.015) return "love";
  if (pnl < -0.015) return "stress";
  if (pnl > 0.004) return "working";
  if (pnl < -0.004) return "alerted";
  if (herding > 0.5 && Math.random() < 0.03) return "chat";
  return "";
}
function hex(c: string): number {
  return parseInt(c.replace("#", ""), 16);
}
