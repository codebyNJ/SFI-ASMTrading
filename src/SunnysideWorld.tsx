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

/* ═══════════════════════════════════════════════════════════════════
   Exported types (consumed by App.tsx)
   ═══════════════════════════════════════════════════════════════════ */
export type TraderView = {
  cohort: number;
  wealth: number;
  position: number;
  pnl: number;
};
export type CohortView = { name: string; color: string; herding: number };

type Data = {
  traders: TraderView[];
  cohorts: CohortView[];
  price: number;
  day: number;
  change: number;
};
type Props = Data & {
  onSelect?: (i: number | null) => void;
  selected?: number | null;
};

/* ═══════════════════════════════════════════════════════════════════
   Constants
   ═══════════════════════════════════════════════════════════════════ */
const PACK = "/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets";
const TS = 3; // display scale: 16 px → 48 px
const MAP_H = 1400; // world height (display px)
const CF = { off: 28, top: 12, w: 40, h: 44, count: 9, stride: 96 };
const PLOT = 360; // horizontal spacing between guild plots
const ROAD_H = 80; // road thickness

/* ═══════════════════════════════════════════════════════════════════
   Asset manifest – every texture the world needs
   ═══════════════════════════════════════════════════════════════════ */
const A: Record<string, string> = {
  // terrain tiles (16×16 native)
  grass: "/world/grass.png",
  dirt: "/world/dirt.png",
  water: "/world/water.png",
  // pre-rendered house sprites (~58×56 native)
  houseBlue: "/world/house_blue.png",
  houseGreen: "/world/house_green.png",
  houseOrange: "/world/house_orange.png",
  housePurple: "/world/house_purple.png",
  houseRed: "/world/house_red.png",
  // character base body
  charBase: `${PACK}/Characters/Human/IDLE/base_idle_strip9.png`,
  // character hair overlays (one per cohort style)
  hairShort: `${PACK}/Characters/Human/IDLE/shorthair_idle_strip9.png`,
  hairBowl: `${PACK}/Characters/Human/IDLE/bowlhair_idle_strip9.png`,
  hairCurly: `${PACK}/Characters/Human/IDLE/curlyhair_idle_strip9.png`,
  hairLong: `${PACK}/Characters/Human/IDLE/longhair_idle_strip9.png`,
  hairSpiky: `${PACK}/Characters/Human/IDLE/spikeyhair_idle_strip9.png`,
  // trees
  tree1: `${PACK}/Elements/Plants/spr_deco_tree_01_strip4.png`,
  tree2: `${PACK}/Elements/Plants/spr_deco_tree_02_strip4.png`,
  // animals
  chicken: `${PACK}/Elements/Animals/spr_deco_chicken_01_strip4.png`,
  cow: `${PACK}/Elements/Animals/spr_deco_cow_strip4.png`,
  sheep: `${PACK}/Elements/Animals/spr_deco_sheep_01_strip4.png`,
  duck: `${PACK}/Elements/Animals/spr_deco_duck_01_strip4.png`,
  bird: `${PACK}/Elements/Animals/spr_deco_bird_01_strip4.png`,
  pig: `${PACK}/Elements/Animals/spr_deco_pig_01_strip4.png`,
  // VFX
  fire1: `${PACK}/Elements/VFX/Fire/spr_deco_fire_01_strip4.png`,
  fire2: `${PACK}/Elements/VFX/Fire/spr_deco_fire_02_strip4.png`,
  smoke: `${PACK}/Elements/VFX/Chimney%20Smoke/chimneysmoke_01_strip30.png`,
  // other elements
  windmill: `${PACK}/Elements/Other/spr_deco_windmill_strip9.png`,
  mushroomR: `${PACK}/Elements/Plants/spr_deco_mushroom_red_01_strip4.png`,
  mushroomB: `${PACK}/Elements/Plants/spr_deco_mushroom_blue_01_strip4.png`,
  coracle: `${PACK}/Elements/Other/spr_deco_coracle_strip4.png`,
  // ambient NPCs
  skeleton: `${PACK}/Characters/Skeleton/PNG/skeleton_idle_strip6.png`,
  // UI expressions (mood bubbles)
  exprLove: `${PACK}/UI/expression_love.png`,
  exprWork: `${PACK}/UI/expression_working.png`,
  exprStress: `${PACK}/UI/expression_stress.png`,
  exprAlert: `${PACK}/UI/expression_alerted.png`,
  exprChat: `${PACK}/UI/expression_chat.png`,
  // crops (fully-grown stage)
  wheat: `${PACK}/Elements/Crops/wheat_04.png`,
  sunflower: `${PACK}/Elements/Crops/sunflower_04.png`,
  pumpkin: `${PACK}/Elements/Crops/pumpkin_04.png`,
  carrot: `${PACK}/Elements/Crops/carrot_04.png`,
};

// cohort-color → house sprite key
const COLOR_HOUSE: Record<string, string> = {
  "#3b82f6": "houseBlue",
  "#22c55e": "houseGreen",
  "#f59e0b": "houseOrange",
  "#a855f7": "housePurple",
  "#ef4444": "houseRed",
};
// cohort-color → hair sprite key
const COLOR_HAIR: Record<string, string> = {
  "#3b82f6": "hairShort",
  "#22c55e": "hairBowl",
  "#f59e0b": "hairCurly",
  "#a855f7": "hairLong",
  "#ef4444": "hairSpiky",
};
const HOUSE_FALLBACK = ["houseBlue", "houseGreen", "houseOrange", "housePurple", "houseRed"];
const HAIR_FALLBACK = ["hairShort", "hairBowl", "hairCurly", "hairLong", "hairSpiky"];

/* ═══════════════════════════════════════════════════════════════════
   Sprite-strip slicer
   ═══════════════════════════════════════════════════════════════════ */
function sliceStrip(
  tex: Texture,
  count: number,
  fw?: number,
  fh?: number,
  ox = 0,
  oy = 0,
): Texture[] {
  tex.source.scaleMode = "nearest";
  const stride = Math.floor(tex.width / count);
  const w = fw ?? stride;
  const h = fh ?? tex.height;
  const out: Texture[] = [];
  for (let i = 0; i < count; i++)
    out.push(
      new Texture({
        source: tex.source,
        frame: new Rectangle(i * stride + ox, oy, w, h),
      }),
    );
  return out;
}
function sliceChar(tex: Texture): Texture[] {
  return sliceStrip(tex, CF.count, CF.w, CF.h, CF.off, CF.top);
}

/* ═══════════════════════════════════════════════════════════════════
   Trader pool item
   ═══════════════════════════════════════════════════════════════════ */
type TraderObj = {
  box: Container;
  pad: Graphics;
  sprite: AnimatedSprite;
  hair: AnimatedSprite;
  bubble: Sprite;
  hx: number;
  hy: number;
  h: number;
  lastLong: number;
  lastBubble: string;
  lastCohort: number;
};

/* ═══════════════════════════════════════════════════════════════════
   COMPONENT
   ═══════════════════════════════════════════════════════════════════ */
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

    /* ── containers ─────────────────────────────────────────────── */
    const world = new Container();
    const ground = new Container(); // terrain tiles
    const nature = new Container(); // trees, animals, decorations
    const statics = new Container(); // houses, tags, board
    const agents = new Container(); // trader characters
    world.addChild(ground, nature, statics, agents);

    /* ── state ───────────────────────────────────────────────────── */
    const pool: TraderObj[] = [];
    const tex: Record<string, Texture> = {};
    const frames: Record<string, Texture[]> = {};
    const moods: Record<string, Texture> = {};
    let boardText: Text | null = null;
    let mapW = 2200;
    let roadY = MAP_H * 0.42;
    let sigPrev = "";
    let lastDay = -1;
    let selRing: Graphics | null = null;

    /* ── seeded RNG (deterministic layout) ───────────────────────── */
    let _seed = 42;
    const rnd = () =>
      ((_seed = (_seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
    const resetRng = () => {
      _seed = 42;
    };

    /* ── helpers ─────────────────────────────────────────────────── */
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
    const plotX = (i: number, n: number) =>
      (mapW - (n - 1) * PLOT) / 2 + i * PLOT;

    /* ── place animated sprite shorthand ─────────────────────────── */
    const placeAnim = (
      parent: Container,
      key: string,
      count: number,
      x: number,
      y: number,
      scale: number,
      speed = 0.06,
      opts?: {
        fw?: number;
        fh?: number;
        ox?: number;
        oy?: number;
        ax?: number;
        ay?: number;
      },
    ): AnimatedSprite | null => {
      if (!tex[key]) return null;
      const f = sliceStrip(
        tex[key],
        count,
        opts?.fw,
        opts?.fh,
        opts?.ox ?? 0,
        opts?.oy ?? 0,
      );
      const s = new AnimatedSprite(f);
      s.anchor.set(opts?.ax ?? 0.5, opts?.ay ?? 0.9);
      s.scale.set(scale);
      s.animationSpeed = speed;
      s.currentFrame = Math.floor(rnd() * count);
      s.play();
      s.position.set(x, y);
      s.zIndex = y;
      parent.addChild(s);
      return s;
    };

    /* ═════════════════════════════════════════════════════════════
       REBUILD – recreate ground + nature + statics
       ═════════════════════════════════════════════════════════════ */
    const rebuild = () => {
      if (!tex.grass || !tex.dirt || !tex.water) return;
      const cohorts = dataRef.current.cohorts;
      const n = Math.max(1, cohorts.length);
      mapW = Math.max(2200, n * PLOT + 500);
      roadY = Math.round(MAP_H * 0.42);
      resetRng();

      const riverY = MAP_H - 350;
      const riverH = 180;
      const farmX = 80;
      const farmY = 200;
      const farmW = 340;
      const farmH = 190;
      const forgeX = mapW - 340;
      const forgeY = roadY + 130;

      /* ── GROUND ──────────────────────────────────────────────── */
      ground.removeChildren();

      // 1. Grass base
      const grass = new TilingSprite({
        texture: tex.grass,
        width: mapW,
        height: MAP_H,
      });
      grass.tileScale.set(TS);
      ground.addChild(grass);

      // 2. Subtle checkerboard overlay
      const checker = new Graphics();
      const step = 16 * TS; // 48 px
      for (let ty = 0; ty < Math.ceil(MAP_H / step); ty++)
        for (let tx = 0; tx < Math.ceil(mapW / step); tx++)
          if ((tx + ty) % 2 === 0)
            checker
              .rect(tx * step, ty * step, step, step)
              .fill({ color: 0x000000, alpha: 0.035 });
      ground.addChild(checker);

      // 3. Horizontal road
      const hRoad = new TilingSprite({
        texture: tex.dirt,
        width: mapW,
        height: ROAD_H,
      });
      hRoad.tileScale.set(TS);
      hRoad.position.set(0, roadY);
      ground.addChild(hRoad);

      // 4. Vertical road
      const vRoadX = mapW / 2 - 40;
      const vRoad = new TilingSprite({
        texture: tex.dirt,
        width: 80,
        height: 520,
      });
      vRoad.tileScale.set(TS);
      vRoad.position.set(vRoadX, roadY - 260);
      ground.addChild(vRoad);

      // 5. Plaza at crossroad
      const plazaW = 220;
      const plazaH = 220;
      const plaza = new TilingSprite({
        texture: tex.dirt,
        width: plazaW,
        height: plazaH,
      });
      plaza.tileScale.set(TS);
      plaza.position.set(
        mapW / 2 - plazaW / 2,
        roadY - (plazaH - ROAD_H) / 2,
      );
      ground.addChild(plaza);

      // 6. Farm soil
      const farmSoil = new TilingSprite({
        texture: tex.dirt,
        width: farmW,
        height: farmH,
      });
      farmSoil.tileScale.set(TS);
      farmSoil.position.set(farmX, farmY);
      ground.addChild(farmSoil);

      // 7. Dirt path: farm → road
      const farmPath = new TilingSprite({
        texture: tex.dirt,
        width: 56,
        height: roadY - farmY - farmH + 10,
      });
      farmPath.tileScale.set(TS);
      farmPath.position.set(farmX + farmW / 2 - 28, farmY + farmH);
      ground.addChild(farmPath);

      // 8. Forge soil
      const forgeSoil = new TilingSprite({
        texture: tex.dirt,
        width: 220,
        height: 160,
      });
      forgeSoil.tileScale.set(TS);
      forgeSoil.position.set(forgeX, forgeY);
      ground.addChild(forgeSoil);

      // 9. Path: forge → road
      const forgePath = new TilingSprite({
        texture: tex.dirt,
        width: 56,
        height: forgeY - roadY - ROAD_H + 10,
      });
      forgePath.tileScale.set(TS);
      forgePath.position.set(forgeX + 80, roadY + ROAD_H);
      ground.addChild(forgePath);

      // 10. River
      const river = new TilingSprite({
        texture: tex.water,
        width: mapW * 0.5,
        height: riverH,
      });
      river.tileScale.set(TS);
      river.position.set(40, riverY);
      ground.addChild(river);

      // 11. Pond (right side)
      const pond = new TilingSprite({
        texture: tex.water,
        width: 260,
        height: 140,
      });
      pond.tileScale.set(TS);
      pond.position.set(mapW - 380, riverY + 20);
      ground.addChild(pond);

      // 12. River / pond banks
      const bank = new Graphics();
      const rw = mapW * 0.5;
      bank.rect(35, riverY - 5, rw + 10, 6).fill(0x6b8c37);
      bank.rect(35, riverY + riverH - 1, rw + 10, 6).fill(0x5a7a2e);
      bank.rect(35, riverY - 5, 6, riverH + 10).fill(0x6b8c37);
      bank.rect(35 + rw + 4, riverY - 5, 6, riverH + 10).fill(0x5a7a2e);
      bank.rect(mapW - 385, riverY + 15, 6, 144).fill(0x6b8c37);
      bank.rect(mapW - 385, riverY + 15, 265, 6).fill(0x6b8c37);
      bank.rect(mapW - 125, riverY + 15, 6, 144).fill(0x5a7a2e);
      bank.rect(mapW - 385, riverY + 153, 265, 6).fill(0x5a7a2e);
      ground.addChild(bank);

      // 13. Bridge over river
      const bridgeX = Math.min(mapW / 2 - 36, 35 + rw - 80);
      const bridge = new Graphics();
      bridge.rect(0, -6, 72, riverH + 12).fill(0x9a7548);
      bridge.rect(-4, -8, 4, riverH + 16).fill(0x7a5230);
      bridge.rect(72, -8, 4, riverH + 16).fill(0x7a5230);
      for (let j = 0; j <= Math.floor(riverH / 20); j++) {
        bridge.rect(-6, j * 20 - 3, 8, 5).fill(0x6a4220);
        bridge.rect(70, j * 20 - 3, 8, 5).fill(0x6a4220);
      }
      for (let j = 0; j < Math.floor(riverH / 8); j++)
        bridge
          .rect(3, j * 8, 66, 1)
          .fill({ color: 0x000000, alpha: 0.08 });
      bridge.position.set(bridgeX, riverY);
      ground.addChild(bridge);

      // 14. Path: bridge → road
      const bridgePath = new TilingSprite({
        texture: tex.dirt,
        width: 60,
        height: riverY - roadY - ROAD_H + 8,
      });
      bridgePath.tileScale.set(TS);
      bridgePath.position.set(bridgeX + 6, roadY + ROAD_H);
      ground.addChild(bridgePath);

      /* ── NATURE (decorations) ────────────────────────────────── */
      nature.removeChildren();
      nature.sortableChildren = true;

      // ── Trees: top forest belt ──
      for (let i = 0; i < 26; i++) {
        const tk = rnd() < 0.6 ? "tree1" : "tree2";
        placeAnim(
          nature,
          tk,
          4,
          30 + rnd() * (mapW - 60),
          35 + rnd() * 110,
          TS * (0.75 + rnd() * 0.2),
          0.03 + rnd() * 0.02,
        );
      }
      // ── Trees: bottom forest belt ──
      for (let i = 0; i < 18; i++) {
        const tk = rnd() < 0.5 ? "tree1" : "tree2";
        placeAnim(
          nature,
          tk,
          4,
          30 + rnd() * (mapW - 60),
          MAP_H - 25 - rnd() * 90,
          TS * (0.7 + rnd() * 0.25),
          0.03 + rnd() * 0.02,
        );
      }
      // ── Trees: left & right edges ──
      for (let i = 0; i < 7; i++) {
        placeAnim(
          nature,
          rnd() < 0.5 ? "tree1" : "tree2",
          4,
          25 + rnd() * 90,
          180 + rnd() * 650,
          TS * 0.8,
          0.035,
        );
      }
      for (let i = 0; i < 7; i++) {
        placeAnim(
          nature,
          rnd() < 0.5 ? "tree1" : "tree2",
          4,
          mapW - 25 - rnd() * 90,
          180 + rnd() * 650,
          TS * 0.8,
          0.035,
        );
      }
      // ── Trees: near river ──
      for (let i = 0; i < 5; i++) {
        const tx = 50 + rnd() * (mapW - 100);
        if (Math.abs(tx - bridgeX - 36) < 100) continue; // don't block bridge
        placeAnim(nature, "tree1", 4, tx, riverY - 15 - rnd() * 70, TS * 0.8, 0.03);
      }

      // ── Fences: farm ──
      nature.addChild(drawFenceRow(farmX, farmY - 4, farmW));
      nature.addChild(drawFenceRow(farmX, farmY + farmH + 2, farmW));
      nature.addChild(drawFenceCol(farmX - 4, farmY, farmH));
      nature.addChild(drawFenceCol(farmX + farmW + 1, farmY, farmH));

      // ── Fences: along river ──
      nature.addChild(drawFenceRow(50, riverY - 14, Math.min(500, rw)));

      // ── Fences: pasture (right upper area) ──
      const pastX = mapW - 500;
      const pastY = 220;
      const pastW = 380;
      const pastH = 200;
      nature.addChild(drawFenceRow(pastX, pastY - 4, pastW));
      nature.addChild(drawFenceRow(pastX, pastY + pastH, pastW));
      nature.addChild(drawFenceCol(pastX - 4, pastY, pastH));
      nature.addChild(drawFenceCol(pastX + pastW + 1, pastY, pastH));

      // ── Crops in farm ──
      const cropKeys = ["wheat", "sunflower", "pumpkin", "carrot"];
      for (let row = 0; row < 4; row++) {
        for (let col = 0; col < 6; col++) {
          const ck = cropKeys[row % cropKeys.length];
          if (!tex[ck]) continue;
          const crop = new Sprite(tex[ck]);
          crop.texture.source.scaleMode = "nearest";
          crop.anchor.set(0.5, 1);
          crop.scale.set(TS);
          crop.position.set(farmX + 35 + col * 50, farmY + 35 + row * 44);
          crop.zIndex = crop.y;
          nature.addChild(crop);
        }
      }

      // ── Animals: chickens near farm ──
      for (let i = 0; i < 4; i++)
        placeAnim(
          nature,
          "chicken",
          4,
          farmX + 40 + rnd() * 260,
          farmY + farmH + 30 + rnd() * 60,
          TS * 0.9,
          0.04 + rnd() * 0.03,
        );

      // ── Animals: cows in pasture ──
      for (let i = 0; i < 3; i++)
        placeAnim(
          nature,
          "cow",
          4,
          pastX + 30 + rnd() * (pastW - 60),
          pastY + 30 + rnd() * (pastH - 60),
          TS * 0.9,
          0.025 + rnd() * 0.02,
        );

      // ── Animals: sheep in pasture ──
      for (let i = 0; i < 3; i++)
        placeAnim(
          nature,
          "sheep",
          4,
          pastX + 40 + rnd() * (pastW - 80),
          pastY + 60 + rnd() * (pastH - 80),
          TS * 0.9,
          0.03,
        );

      // ── Animals: pigs near farm ──
      for (let i = 0; i < 2; i++)
        placeAnim(
          nature,
          "pig",
          4,
          farmX + 20 + rnd() * 180,
          farmY + farmH + 60 + rnd() * 40,
          TS * 0.85,
          0.035,
        );

      // ── Animals: ducks on river ──
      for (let i = 0; i < 4; i++)
        placeAnim(
          nature,
          "duck",
          4,
          80 + rnd() * (rw - 60),
          riverY + 20 + rnd() * (riverH - 40),
          TS,
          0.055,
        );

      // ── Animals: birds (sky) ──
      for (let i = 0; i < 3; i++)
        placeAnim(
          nature,
          "bird",
          4,
          150 + rnd() * (mapW - 300),
          25 + rnd() * 80,
          TS,
          0.07,
        );

      // ── Windmill ──
      placeAnim(nature, "windmill", 9, 210, roadY - 40, TS * 0.85, 0.055, {
        ax: 0.5,
        ay: 0.85,
      });

      // ── Fire at forge ──
      placeAnim(nature, "fire2", 4, forgeX + 90, forgeY + 85, TS * 2.5, 0.1);
      placeAnim(nature, "fire1", 4, forgeX + 120, forgeY + 95, TS * 2, 0.12);

      // ── Forge anvil (drawn) ──
      const anvil = new Graphics();
      anvil
        .roundRect(-10, -8, 20, 12, 2)
        .fill(0x555555)
        .stroke({ color: 0x333333, width: 1 });
      anvil.rect(-5, 4, 10, 6).fill(0x444444);
      anvil.position.set(forgeX + 55, forgeY + 65);
      anvil.scale.set(TS * 0.5);
      anvil.zIndex = forgeY + 65;
      nature.addChild(anvil);

      // ── Mushrooms ──
      for (let i = 0; i < 8; i++) {
        const mk = rnd() < 0.5 ? "mushroomR" : "mushroomB";
        const mx = 40 + rnd() * (mapW - 80);
        const my =
          rnd() < 0.5 ? 50 + rnd() * 80 : MAP_H - 50 - rnd() * 80;
        placeAnim(nature, mk, 4, mx, my, TS, 0.02);
      }

      // ── Coracle / boat ──
      placeAnim(nature, "coracle", 4, 75, riverY - 8, TS * 0.9, 0.025);

      // ── Skeleton NPCs ──
      for (let i = 0; i < 4; i++) {
        const sx = 120 + rnd() * (mapW - 240);
        const sy =
          rnd() < 0.5
            ? MAP_H - 60 - rnd() * 90
            : roadY + 200 + rnd() * 200;
        placeAnim(nature, "skeleton", 6, sx, sy, TS * 0.65, 0.045, {
          fw: CF.w,
          fh: CF.h,
          ox: CF.off,
          oy: CF.top,
        });
      }

      // ── Flowers (batched) ──
      const flowerCol = [0xffffff, 0xff6b8a, 0xffee44, 0xff9955, 0xbb88ff];
      const flowers = new Graphics();
      for (let i = 0; i < 50; i++) {
        const fx = 40 + rnd() * (mapW - 80);
        const fy = 70 + rnd() * (MAP_H - 180);
        if (fy > roadY - 8 && fy < roadY + ROAD_H + 8) continue;
        if (fy > riverY - 8 && fy < riverY + riverH + 8) continue;
        if (fx > farmX - 5 && fx < farmX + farmW + 5 && fy > farmY - 5 && fy < farmY + farmH + 5) continue;
        const c = flowerCol[Math.floor(rnd() * flowerCol.length)];
        // stem
        flowers.rect(fx - 0.5, fy, 1, 4).fill(0x4a8c3c);
        // petals
        flowers.circle(fx, fy - 1, 2.5).fill(c);
        flowers.circle(fx, fy - 1, 1).fill(0xffee44);
      }
      flowers.scale.set(TS * 0.45);
      flowers.zIndex = 0;
      nature.addChild(flowers);

      // ── Rocks (batched) ──
      const rocks = new Graphics();
      for (let i = 0; i < 18; i++) {
        const rx = 50 + rnd() * (mapW - 100);
        const ry = 90 + rnd() * (MAP_H - 250);
        if (ry > roadY - 6 && ry < roadY + ROAD_H + 6) continue;
        if (ry > riverY - 6 && ry < riverY + riverH + 6) continue;
        const rs = 3 + rnd() * 5;
        rocks.ellipse(rx, ry, rs, rs * 0.65).fill(0x8a8a8a);
        rocks
          .ellipse(rx - rs * 0.25, ry - rs * 0.2, rs * 0.4, rs * 0.3)
          .fill(0x9e9e9e);
      }
      rocks.scale.set(TS * 0.4);
      rocks.zIndex = 1;
      nature.addChild(rocks);

      /* ── STATICS (houses, tags, board) ───────────────────────── */
      statics.removeChildren();
      statics.sortableChildren = true;

      cohorts.forEach((c, i) => {
        const cx = plotX(i, n);
        const houseKey =
          COLOR_HOUSE[c.color] ?? HOUSE_FALLBACK[i % HOUSE_FALLBACK.length];

        // ── House sprite ──
        if (tex[houseKey]) {
          const house = new Sprite(tex[houseKey]);
          house.anchor.set(0.5, 0.95);
          house.scale.set(TS);
          house.position.set(cx, roadY - 2);
          house.zIndex = roadY - 2;
          statics.addChild(house);

          // ── Chimney smoke ──
          if (tex.smoke) {
            try {
              const sf = sliceStrip(tex.smoke, 30);
              const sm = new AnimatedSprite(sf);
              sm.anchor.set(0.5, 1);
              sm.scale.set(TS * 1.2);
              sm.animationSpeed = 0.14;
              sm.currentFrame = Math.floor(rnd() * 30);
              sm.play();
              sm.position.set(cx + 40, roadY - 148);
              sm.zIndex = roadY - 300;
              sm.alpha = 0.7;
              statics.addChild(sm);
            } catch {
              /* smoke texture layout mismatch – skip */
            }
          }
        } else {
          // fallback: programmatic cottage
          const cottage = drawCottage(hex(c.color));
          cottage.position.set(cx, roadY - 6);
          cottage.zIndex = roadY - 6;
          statics.addChild(cottage);
        }

        // ── Name tag ──
        const tag = new Container();
        const bg = new Graphics();
        bg.roundRect(-58, -17, 116, 28, 8).fill({
          color: hex(c.color),
          alpha: 0.92,
        });
        bg.roundRect(-58, -17, 116, 28, 8).stroke({
          color: 0x000000,
          width: 1,
          alpha: 0.12,
        });
        const t = new Text({
          text: c.name,
          style: {
            fill: "#ffffff",
            fontSize: 13,
            fontFamily: "system-ui, sans-serif",
            fontWeight: "700",
          },
        });
        t.anchor.set(0.5);
        t.position.set(0, -3);
        tag.addChild(bg, t);
        tag.position.set(cx, roadY - 180);
        tag.zIndex = 1e6;
        statics.addChild(tag);
      });

      // ── Price / notice board ──
      const board = new Container();
      const postG = new Graphics();
      postG.rect(-3, 10, 6, 30).fill(0x7a5230);
      postG.rect(-5, 38, 10, 5).fill(0x6a4220);
      board.addChild(postG);
      const face = new Graphics();
      face
        .roundRect(-60, -42, 120, 54, 6)
        .fill(0xf5eed6)
        .stroke({ color: 0x8b5a2b, width: 3 });
      face.rect(-60, -8, 120, 3).fill(0x8b5a2b);
      board.addChild(face);
      boardText = new Text({
        text: "",
        style: {
          fill: "#0f172a",
          fontSize: 20,
          fontFamily: "system-ui, monospace",
          fontWeight: "700",
        },
      });
      boardText.anchor.set(0.5);
      boardText.position.set(0, -20);
      board.addChild(boardText);
      const lbl = new Text({
        text: "MARKET",
        style: {
          fill: "#7a5230",
          fontSize: 8,
          fontFamily: "system-ui",
          fontWeight: "600",
          letterSpacing: 2,
        },
      });
      lbl.anchor.set(0.5);
      lbl.position.set(0, 0);
      board.addChild(lbl);
      board.position.set(mapW / 2, roadY + 56);
      board.zIndex = 1e6;
      statics.addChild(board);

      /* ── TRADER HOME SLOTS ───────────────────────────────────── */
      ensurePool(dataRef.current.traders.length);
      const counts = new Array(n).fill(0);
      for (const tr of dataRef.current.traders)
        counts[Math.min(tr.cohort, n - 1)]++;
      const colsArr = counts.map((c: number) =>
        Math.max(1, Math.ceil(Math.sqrt(c))),
      );
      const seen = new Array(n).fill(0);
      const cell = 36;
      for (
        let i = 0;
        i < dataRef.current.traders.length && i < pool.length;
        i++
      ) {
        const ci = Math.min(dataRef.current.traders[i].cohort, n - 1);
        const slot = seen[ci]++;
        const nc = colsArr[ci];
        const col = slot % nc;
        const row = Math.floor(slot / nc);
        const gw = (nc - 1) * cell;
        pool[i].hx = plotX(ci, n) - gw / 2 + col * cell;
        pool[i].hy = roadY + ROAD_H + 30 + row * cell;

        // assign hair per cohort
        const cColor = cohorts[ci]?.color ?? "";
        const hairKey =
          COLOR_HAIR[cColor] ?? HAIR_FALLBACK[ci % HAIR_FALLBACK.length];
        if (frames[hairKey] && pool[i].lastCohort !== ci) {
          pool[i].lastCohort = ci;
          pool[i].hair.textures = frames[hairKey];
          pool[i].hair.play();
        }
      }
    };

    /* ═════════════════════════════════════════════════════════════
       ENSURE POOL – create / destroy trader sprites
       ═════════════════════════════════════════════════════════════ */
    const ensurePool = (count: number) => {
      while (pool.length < count) {
        const box = new Container();
        const pad = new Graphics();

        // body
        const sprite = new AnimatedSprite(
          frames.charBase ?? [Texture.WHITE],
        );
        sprite.anchor.set(0.5, 0.92);
        sprite.animationSpeed = 0.08;
        sprite.currentFrame = Math.floor(
          Math.random() * (frames.charBase?.length ?? 1),
        );
        sprite.play();

        // hair overlay
        const hair = new AnimatedSprite(
          frames.hairShort ?? frames.charBase ?? [Texture.WHITE],
        );
        hair.anchor.set(0.5, 0.92);
        hair.animationSpeed = 0.08;
        hair.currentFrame = sprite.currentFrame;
        hair.play();

        // mood bubble
        const bubble = new Sprite();
        bubble.anchor.set(0.5, 1);
        bubble.visible = false;

        box.addChild(pad, sprite, hair, bubble);
        const myIndex = pool.length;
        box.eventMode = "static";
        box.cursor = "pointer";
        box.hitArea = new Rectangle(-18, -56, 36, 60);
        box.on("pointerdown", (e) => e.stopPropagation());
        box.on("pointertap", (e) => {
          e.stopPropagation();
          onSelectRef.current?.(myIndex);
        });
        agents.addChild(box);
        pool.push({
          box,
          pad,
          sprite,
          hair,
          bubble,
          hx: mapW / 2,
          hy: roadY + ROAD_H + 30,
          h: 44,
          lastLong: 2,
          lastBubble: "",
          lastCohort: -1,
        });
      }
      while (pool.length > count)
        pool.pop()!.box.destroy({ children: true });
    };

    /* ═════════════════════════════════════════════════════════════
       UPDATE DAY – apply trader state changes
       ═════════════════════════════════════════════════════════════ */
    const updateDay = () => {
      const { traders, cohorts, price, change } = dataRef.current;
      if (boardText) {
        boardText.text = price > 0 ? `$${price.toFixed(2)}` : "—";
        boardText.style.fill =
          change > 0 ? "#15803d" : change < 0 ? "#b91c1c" : "#0f172a";
      }
      let wMin = Infinity,
        wMax = -Infinity;
      for (const t of traders) {
        wMin = Math.min(wMin, t.wealth);
        wMax = Math.max(wMax, t.wealth);
      }
      const wSpan = Math.max(1e-6, wMax - wMin);
      for (let i = 0; i < traders.length && i < pool.length; i++) {
        const d = traders[i],
          tr = pool[i];
        tr.h = 40 + 26 * ((d.wealth - wMin) / wSpan);
        const long = d.position >= 0 ? 1 : 0;
        if (long !== tr.lastLong) {
          tr.lastLong = long;
          tr.pad.clear();
          tr.pad
            .ellipse(0, 2, 17, 7)
            .fill({ color: long ? 0x22c55e : 0xef4444, alpha: 0.5 });
        }
        const b = bubbleFor(d.pnl, cohorts[d.cohort]?.herding ?? 0);
        if (b !== tr.lastBubble) {
          tr.lastBubble = b;
          if (b && moods[b]) {
            tr.bubble.texture = moods[b];
            tr.bubble.visible = true;
            tr.bubble.height = 16;
            tr.bubble.width =
              16 * (moods[b].width / Math.max(1, moods[b].height));
          } else tr.bubble.visible = false;
        }
        tr.box.zIndex = tr.hy;
      }
      agents.sortChildren();
    };

    /* ═════════════════════════════════════════════════════════════
       TICK – per-frame animation
       ═════════════════════════════════════════════════════════════ */
    const tick = () => {
      if (!appRef.current || !frames.charBase?.length) return;
      const data = dataRef.current;

      // detect cohort / trader changes → full rebuild
      const sig =
        data.cohorts.map((c) => c.name + c.color).join("|") +
        "#" +
        data.traders.length;
      if (sig !== sigPrev) {
        sigPrev = sig;
        rebuild();
        centerOn(mapW / 2, roadY);
        lastDay = -1;
      }
      if (data.day !== lastDay) {
        lastDay = data.day;
        updateDay();
      }

      const t = performance.now() / 1000;
      for (let i = 0; i < data.traders.length && i < pool.length; i++) {
        const tr = pool[i];
        // gentle idle sway
        tr.box.x = tr.hx + Math.sin(t * 0.5 + i * 1.7) * 6;
        tr.box.y = tr.hy + Math.cos(t * 0.42 + i * 2.3) * 4;
        const face = data.traders[i].position >= 0 ? 1 : -1;
        const sc = tr.h / CF.h;
        tr.sprite.scale.set(sc * face, sc);
        tr.hair.scale.set(sc * face, sc);
        // keep hair synced with body
        if (tr.hair.currentFrame !== tr.sprite.currentFrame)
          tr.hair.currentFrame = tr.sprite.currentFrame;
        tr.bubble.position.set(0, -tr.h - 2);
      }

      // selection ring
      const sel = selectedRef.current;
      if (selRing) {
        if (
          sel != null &&
          sel >= 0 &&
          sel < pool.length &&
          sel < data.traders.length
        ) {
          const tr = pool[sel];
          selRing.clear();
          selRing
            .circle(0, -tr.h * 0.32, tr.h * 0.5)
            .stroke({ color: 0x2563eb, width: 3, alpha: 0.95 });
          selRing.position.set(tr.box.x, tr.box.y);
          selRing.visible = true;
        } else {
          selRing.visible = false;
        }
      }
    };

    /* ═════════════════════════════════════════════════════════════
       APP INIT + ASSET LOADING
       ═════════════════════════════════════════════════════════════ */
    app
      .init({
        background: 0x5a9a2a,
        antialias: false,
        autoDensity: true,
        resolution: window.devicePixelRatio || 1,
        width: host.clientWidth || 480,
        height: host.clientHeight || 360,
      })
      .then(async () => {
        if (destroyed) {
          app.destroy(true, { children: true });
          return;
        }
        appRef.current = app;
        app.ticker.maxFPS = 30;
        host.appendChild(app.canvas);
        agents.sortableChildren = true;
        app.stage.addChild(world);

        // ── load every asset in parallel ──
        const entries = Object.entries(A);
        await Promise.all(
          entries.map(async ([key, url]) => {
            try {
              const t = (await Assets.load(url)) as Texture;
              t.source.scaleMode = "nearest";
              tex[key] = t;
            } catch {
              // non-critical – world renders without this asset
            }
          }),
        );
        if (destroyed) return;

        // ── prepare character frame arrays ──
        if (tex.charBase) frames.charBase = sliceChar(tex.charBase);
        for (const hk of [
          "hairShort",
          "hairBowl",
          "hairCurly",
          "hairLong",
          "hairSpiky",
        ]) {
          if (tex[hk]) frames[hk] = sliceChar(tex[hk]);
        }

        // ── mood textures ──
        if (tex.exprLove) moods.love = tex.exprLove;
        if (tex.exprWork) moods.working = tex.exprWork;
        if (tex.exprStress) moods.stress = tex.exprStress;
        if (tex.exprAlert) moods.alerted = tex.exprAlert;
        if (tex.exprChat) moods.chat = tex.exprChat;

        // ── pan / drag ──
        app.stage.eventMode = "static";
        app.stage.hitArea = app.screen;
        let drag = false,
          sx = 0,
          sy = 0,
          ox = 0,
          oy = 0;
        app.stage.on("pointerdown", (e) => {
          drag = true;
          sx = e.global.x;
          sy = e.global.y;
          ox = world.x;
          oy = world.y;
        });
        app.stage.on("pointermove", (e) => {
          if (!drag) return;
          world.x = ox + (e.global.x - sx);
          world.y = oy + (e.global.y - sy);
          clampPan();
        });
        const stop = () => {
          drag = false;
        };
        app.stage.on("pointerup", stop);
        app.stage.on("pointerupoutside", stop);
        app.stage.on("pointertap", () => onSelectRef.current?.(null));

        // ── selection ring ──
        selRing = new Graphics();
        selRing.visible = false;
        world.addChild(selRing);

        // ── resize ──
        const resize = () => {
          app.renderer.resize(host.clientWidth, host.clientHeight);
          clampPan();
        };
        resize();
        ro = new ResizeObserver(resize);
        ro.observe(host);

        // ── start ──
        app.ticker.add(tick);
      });

    /* ── cleanup ─────────────────────────────────────────────────── */
    return () => {
      destroyed = true;
      ro?.disconnect();
      const a = appRef.current;
      if (a) {
        a.ticker.remove(tick);
        a.destroy(true, { children: true });
      }
      appRef.current = null;
    };
  }, []);

  return (
    <div
      ref={hostRef}
      className="h-full w-full cursor-grab overflow-hidden rounded-lg active:cursor-grabbing"
    />
  );
}

/* ═══════════════════════════════════════════════════════════════════
   DRAWING HELPERS
   ═══════════════════════════════════════════════════════════════════ */

/** Horizontal fence row */
function drawFenceRow(x: number, y: number, width: number): Graphics {
  const g = new Graphics();
  const gap = 22;
  const posts = Math.floor(width / gap);
  for (let i = 0; i <= posts; i++) {
    g.rect(i * gap - 2, -12, 4, 16).fill(0x8b5e2b);
    g.rect(i * gap - 3, -14, 6, 3).fill(0xa07040);
  }
  g.rect(0, -8, width, 2).fill(0x9a6a40);
  g.rect(0, -1, width, 2).fill(0x9a6a40);
  g.position.set(x, y);
  g.zIndex = y;
  return g;
}

/** Vertical fence column */
function drawFenceCol(x: number, y: number, height: number): Graphics {
  const g = new Graphics();
  const gap = 22;
  const posts = Math.floor(height / gap);
  for (let i = 0; i <= posts; i++) {
    g.rect(-2, i * gap - 2, 4, 4).fill(0x8b5e2b);
    g.rect(-3, i * gap - 3, 6, 6).fill(0xa07040);
  }
  g.rect(-1, 0, 2, height).fill(0x9a6a40);
  g.rect(1, 0, 2, height).fill(0x8a5a30);
  g.position.set(x, y);
  g.zIndex = y + height;
  return g;
}

/** Fallback cottage (same as original – used when house sprite fails) */
function drawCottage(roof: number): Graphics {
  const g = new Graphics();
  g.roundRect(-30, -42, 60, 46, 3)
    .fill({ color: 0xe7d3ab })
    .stroke({ color: 0x7a5230, width: 2 });
  g.moveTo(-38, -38)
    .lineTo(0, -70)
    .lineTo(38, -38)
    .closePath()
    .fill({ color: roof })
    .stroke({ color: 0x5c3a1e, width: 2 });
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
