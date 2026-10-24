//! HumanIK-style character setup: a standard humanoid skeleton, character definitions that map a
//! skeleton's joints to named body slots (found automatically from common naming schemes), a
//! control rig (FK body, IK arms and legs with pole vectors), and live retargeting from one
//! character to another, which can be baked to keys.
//!
//! Retargeting compares each joint's world rotation with its rotation in the stance (T-pose) the
//! character was defined in, so both characters should be defined in a T-pose facing +Z.

use crate::math::DVec3;
use crate::scene::{Id, Scene};
use glam::{DMat4, DQuat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Body slots in parent-before-child order, with their group in the Character Controls window.
pub const SLOTS: &[(&str, &str)] = &[
    ("Reference", "Center"),
    ("Hips", "Center"),
    ("Spine", "Center"),
    ("Spine1", "Center"),
    ("Spine2", "Center"),
    ("Spine3", "Center"),
    ("Spine4", "Center"),
    ("Spine5", "Center"),
    ("Spine6", "Center"),
    ("Spine7", "Center"),
    ("Spine8", "Center"),
    ("Spine9", "Center"),
    ("Neck", "Center"),
    ("Neck1", "Center"),
    ("Neck2", "Center"),
    ("Head", "Center"),
    ("LeftShoulder", "Left Arm"),
    ("LeftArm", "Left Arm"),
    ("LeftForeArm", "Left Arm"),
    ("LeftHand", "Left Arm"),
    ("RightShoulder", "Right Arm"),
    ("RightArm", "Right Arm"),
    ("RightForeArm", "Right Arm"),
    ("RightHand", "Right Arm"),
    ("LeftUpLeg", "Left Leg"),
    ("LeftLeg", "Left Leg"),
    ("LeftFoot", "Left Leg"),
    ("LeftToeBase", "Left Leg"),
    ("RightUpLeg", "Right Leg"),
    ("RightLeg", "Right Leg"),
    ("RightFoot", "Right Leg"),
    ("RightToeBase", "Right Leg"),
    ("LeftHandThumb1", "Left Hand"),
    ("LeftHandThumb2", "Left Hand"),
    ("LeftHandThumb3", "Left Hand"),
    ("LeftHandIndex1", "Left Hand"),
    ("LeftHandIndex2", "Left Hand"),
    ("LeftHandIndex3", "Left Hand"),
    ("LeftHandMiddle1", "Left Hand"),
    ("LeftHandMiddle2", "Left Hand"),
    ("LeftHandMiddle3", "Left Hand"),
    ("LeftHandRing1", "Left Hand"),
    ("LeftHandRing2", "Left Hand"),
    ("LeftHandRing3", "Left Hand"),
    ("LeftHandPinky1", "Left Hand"),
    ("LeftHandPinky2", "Left Hand"),
    ("LeftHandPinky3", "Left Hand"),
    ("RightHandThumb1", "Right Hand"),
    ("RightHandThumb2", "Right Hand"),
    ("RightHandThumb3", "Right Hand"),
    ("RightHandIndex1", "Right Hand"),
    ("RightHandIndex2", "Right Hand"),
    ("RightHandIndex3", "Right Hand"),
    ("RightHandMiddle1", "Right Hand"),
    ("RightHandMiddle2", "Right Hand"),
    ("RightHandMiddle3", "Right Hand"),
    ("RightHandRing1", "Right Hand"),
    ("RightHandRing2", "Right Hand"),
    ("RightHandRing3", "Right Hand"),
    ("RightHandPinky1", "Right Hand"),
    ("RightHandPinky2", "Right Hand"),
    ("RightHandPinky3", "Right Hand"),
];

/// Slots a definition needs before it can drive or be driven.
pub const REQUIRED: &[&str] = &[
    "Hips", "Spine", "Head", "LeftUpLeg", "LeftLeg", "LeftFoot", "RightUpLeg", "RightLeg", "RightFoot", "LeftArm", "LeftForeArm", "LeftHand", "RightArm", "RightForeArm", "RightHand",
];

pub const SOURCE_CONTROL_RIG: &str = "Control Rig";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRig {
    /// the rig's top control (Reference)
    pub root: Id,
    /// slot (or "<slot>Pole") -> control node
    pub controls: BTreeMap<String, Id>,
    pub ik_handles: Vec<Id>,
    /// names of the constraints the rig made
    pub constraints: Vec<String>,
    /// hand / foot slot -> rotation from the effector control to the joint (x, y, z, w)
    pub offsets: BTreeMap<String, [f64; 4]>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub name: String,
    /// slot -> joint
    pub joints: BTreeMap<String, Id>,
    /// slot -> world rotation in the stance pose (x, y, z, w)
    pub stance: BTreeMap<String, [f64; 4]>,
    /// world position of the hips in the stance pose
    pub stance_hips: [f64; 3],
    /// "" (none), [`SOURCE_CONTROL_RIG`], or the name of the character that drives this one
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub rig: Option<ControlRig>,
}

impl Character {
    pub fn missing(&self) -> Vec<&'static str> {
        REQUIRED.iter().copied().filter(|r| !self.joints.contains_key(*r)).collect()
    }
}

fn q4(q: DQuat) -> [f64; 4] {
    q.to_array()
}
fn quat(a: &[f64; 4]) -> DQuat {
    DQuat::from_array(*a).normalize()
}

// ---------------------------------------------------------------- name matching

/// Lowercase words of a joint name: namespaces dropped, split at separators, case changes and digits.
fn tokens(name: &str) -> Vec<String> {
    let base = name.rsplit([':', '|']).next().unwrap_or(name);
    let c: Vec<char> = base.chars().collect();
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    for (i, &ch) in c.iter().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let prev = if i > 0 { Some(c[i - 1]) } else { None };
        let split = match prev {
            Some(p) if p.is_ascii_alphanumeric() => (p.is_ascii_lowercase() && ch.is_ascii_uppercase()) || (p.is_ascii_digit() != ch.is_ascii_digit()) || (p.is_ascii_uppercase() && ch.is_ascii_uppercase() && c.get(i + 1).is_some_and(|n| n.is_ascii_lowercase())),
            _ => false,
        };
        if split && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch.to_ascii_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The slot a joint name suggests (spine and neck chains are found from the hierarchy instead).
fn classify(name: &str) -> Option<String> {
    let t = tokens(name);
    if t.iter().any(|w| matches!(w.as_str(), "end" | "nub" | "top" | "tip" | "twist" | "roll" | "ik" | "fk" | "ctrl" | "pole")) {
        return None;
    }
    let side = if t.iter().any(|w| w == "left" || w == "l" || w == "lf" || w == "lt") {
        Some("Left")
    } else if t.iter().any(|w| w == "right" || w == "r" || w == "rt" || w == "rf") {
        Some("Right")
    } else {
        None
    };
    let digit: Option<u32> = t.iter().rev().find_map(|w| w.parse::<u32>().ok());
    let junk = ["left", "right", "l", "r", "lf", "rf", "lt", "rt", "mixamorig", "bip", "def", "jnt", "joint", "bn", "b", "c", "j", "org", "mch", "drv", "skin", "bind", "sk", "bone"];
    let words: Vec<&str> = t.iter().map(String::as_str).filter(|w| !junk.contains(w) && w.parse::<u32>().is_err()).collect();
    let zero_based = t.iter().any(|w| w.len() > 1 && w.parse::<u32>() == Ok(0));
    // a prefix (the character's name, a rig name) is dropped word by word until something matches
    (0..words.len()).find_map(|k| slot_of(&words[k..].concat(), side, digit, zero_based))
}

fn slot_of(part: &str, side: Option<&str>, digit: Option<u32>, zero_based: bool) -> Option<String> {
    let Some(side) = side else {
        return match part {
            "hips" | "pelvis" | "hip" | "cog" => Some("Hips".into()),
            "head" => Some("Head".into()),
            "neck" => Some("Neck".into()),
            _ => None,
        };
    };
    // fingers: thumb / index / middle / ring / pinky, numbered from the hand outwards
    let finger = ["thumb", "index", "middle", "ring", "pinky", "little", "pointer", "mid"].iter().find(|f| part.trim_start_matches("hand").trim_start_matches("finger").trim_start_matches("f").starts_with(**f)).copied();
    if let Some(f) = finger {
        let f = match f {
            "little" => "Pinky".to_string(),
            "pointer" => "Index".to_string(),
            "mid" => "Middle".to_string(),
            f => format!("{}{}", f[..1].to_uppercase(), &f[1..]),
        };
        let n = digit.unwrap_or(1);
        // zero-based numbering (finger_00) shifts by one
        let n = if zero_based { n + 1 } else { n };
        return (1..=3).contains(&n).then(|| format!("{side}Hand{f}{n}"));
    }
    let slot = match part {
        "upleg" | "thigh" | "upperleg" | "legupper" | "femur" | "hip" | "uleg" => "UpLeg",
        "leg" | "calf" | "shin" | "knee" | "lowerleg" | "loleg" | "leglower" | "lleg" => "Leg",
        "foot" | "ankle" => "Foot",
        "toebase" | "toe" | "toes" | "ball" => "ToeBase",
        "shoulder" | "clavicle" | "collar" | "clav" => "Shoulder",
        "arm" | "upperarm" | "uparm" | "shldr" | "humerus" | "armupper" | "uarm" => "Arm",
        "forearm" | "lowerarm" | "elbow" | "loarm" | "armlower" | "radius" | "farm" => "ForeArm",
        "hand" | "wrist" => "Hand",
        _ => return None,
    };
    Some(format!("{side}{slot}"))
}

impl Scene {
    fn hik_index(&self, name: &str) -> Option<usize> {
        self.characters.iter().position(|c| c.name == name)
    }
    pub fn hik_character(&self, name: &str) -> Option<&Character> {
        self.characters.iter().find(|c| c.name == name)
    }
    fn hik_unique(&self, want: &str) -> String {
        let mut n = want.to_string();
        let mut i = 1;
        while self.hik_character(&n).is_some() {
            i += 1;
            n = format!("{want}{i}");
        }
        n
    }
    fn joint_depth(&self, id: &str) -> usize {
        let mut d = 0;
        let mut c = self.node(id).and_then(|n| n.parent.clone());
        while let Some(p) = c {
            d += 1;
            c = self.node(&p).and_then(|n| n.parent.clone());
        }
        d
    }
    fn first_child_joint(&self, id: &str) -> Option<Id> {
        self.children(Some(id)).iter().find(|c| self.node(c).is_some_and(|n| n.kind == "joint")).cloned()
    }
    /// Map the joints under `root` to body slots from their names and the hierarchy.
    pub fn hik_auto_map(&self, root: &str) -> BTreeMap<String, Id> {
        let joints: Vec<Id> = self.subtree(root).into_iter().filter(|j| self.node(j).is_some_and(|n| n.kind == "joint")).collect();
        let mut map: BTreeMap<String, Id> = BTreeMap::new();
        for j in &joints {
            if let Some(slot) = classify(&self.name_of(j)) {
                // the first (closest to the root) joint wins
                map.entry(slot).or_insert_with(|| j.clone());
            }
        }
        // limbs: fill the rest of the chain from the hierarchy
        for side in ["Left", "Right"] {
            for chain in [["UpLeg", "Leg", "Foot", "ToeBase"], ["Arm", "ForeArm", "Hand", ""]] {
                for i in 1..chain.len() {
                    if chain[i].is_empty() {
                        continue;
                    }
                    let (prev, cur) = (format!("{side}{}", chain[i - 1]), format!("{side}{}", chain[i]));
                    if !map.contains_key(&cur) {
                        if let Some(c) = map.get(&prev).and_then(|p| self.first_child_joint(p)) {
                            map.insert(cur, c);
                        }
                    }
                }
            }
            // an arm without a shoulder: a sided parent joint between the arm and the spine
            let (sh, arm) = (format!("{side}Shoulder"), format!("{side}Arm"));
            if !map.contains_key(&sh) {
                if let Some(p) = map.get(&arm).and_then(|a| self.node(a)).and_then(|n| n.parent.clone()) {
                    if classify(&self.name_of(&p)).is_none() && tokens(&self.name_of(&p)).iter().any(|w| w == "l" || w == "r" || w == "left" || w == "right") {
                        map.insert(sh, p);
                    }
                }
            }
        }
        // hips: the joint above both legs
        if !map.contains_key("Hips") {
            if let (Some(l), Some(r)) = (map.get("LeftUpLeg"), map.get("RightUpLeg")) {
                let mut c = self.node(l).and_then(|n| n.parent.clone());
                while let Some(p) = c {
                    if self.is_ancestor(&p, r) && self.node(&p).is_some_and(|n| n.kind == "joint") {
                        map.insert("Hips".into(), p);
                        break;
                    }
                    c = self.node(&p).and_then(|n| n.parent.clone());
                }
            }
        }
        // spine: the joints from the hips up to the neck (or to the joint the arms hang from)
        let Some(hips) = map.get("Hips").cloned() else { return map };
        let top = map.get("Neck").and_then(|n| self.node(n)).and_then(|n| n.parent.clone()).or_else(|| {
            let arm_root = map.get("LeftShoulder").or(map.get("LeftArm"))?;
            self.node(arm_root)?.parent.clone()
        });
        if let Some(top) = top.filter(|t| self.is_ancestor(&hips, t)) {
            let mut chain = vec![];
            let mut c = Some(top);
            while let Some(p) = c.filter(|p| *p != hips) {
                chain.insert(0, p.clone());
                c = self.node(&p).and_then(|n| n.parent.clone());
            }
            map.retain(|k, v| !(k.starts_with("Spine") || (chain.contains(v) && k != "Hips")));
            for (i, j) in chain.iter().take(10).enumerate() {
                map.insert(if i == 0 { "Spine".into() } else { format!("Spine{i}") }, j.clone());
            }
        }
        // neck chain: Neck .. the head's parent
        if let (Some(neck), Some(head)) = (map.get("Neck").cloned(), map.get("Head").cloned()) {
            let mut chain = vec![];
            let mut c = self.node(&head).and_then(|n| n.parent.clone());
            while let Some(p) = c.filter(|p| *p != neck && self.is_ancestor(&neck, p)) {
                chain.insert(0, p.clone());
                c = self.node(&p).and_then(|n| n.parent.clone());
            }
            for (i, j) in chain.iter().take(2).enumerate() {
                map.insert(format!("Neck{}", i + 1), j.clone());
            }
        }
        if let Some(p) = self.node(&hips).and_then(|n| n.parent.clone()) {
            map.entry("Reference".into()).or_insert(p);
        }
        map
    }

    /// Record the current pose as the character's stance.
    fn hik_capture_stance(&mut self, i: usize) {
        let joints = self.characters[i].joints.clone();
        let stance: BTreeMap<String, [f64; 4]> = joints.iter().filter(|(_, j)| self.nodes.contains_key(*j)).map(|(k, j)| (k.clone(), q4(self.world_matrix(j).to_scale_rotation_translation().1))).collect();
        let hips = joints.get("Hips").map(|h| self.world_pos(h)).unwrap_or_default();
        let c = &mut self.characters[i];
        c.stance = stance;
        c.stance_hips = hips.to_array();
    }

    /// Define a character from the skeleton under `root` (auto-mapped); returns its name.
    pub fn hik_define(&mut self, root: &str, name: Option<&str>) -> Result<String, String> {
        if !self.node(root).is_some_and(|n| n.kind == "joint" || !self.subtree(root).is_empty()) {
            return Err("Select the skeleton's root joint".into());
        }
        let joints = self.hik_auto_map(root);
        let name = self.hik_unique(name.unwrap_or("Character1"));
        self.characters.push(Character { name: name.clone(), joints, ..Default::default() });
        let i = self.characters.len() - 1;
        self.hik_capture_stance(i);
        self.touch();
        Ok(name)
    }

    /// Assign (or clear) one slot of a definition; the joint's current pose becomes its stance.
    pub fn hik_set_slot(&mut self, ch: &str, slot: &str, joint: Option<&str>) {
        let Some(i) = self.hik_index(ch) else { return };
        match joint {
            Some(j) => {
                let q = q4(self.world_matrix(j).to_scale_rotation_translation().1);
                let p = self.world_pos(j);
                let c = &mut self.characters[i];
                c.joints.insert(slot.into(), j.into());
                c.stance.insert(slot.into(), q);
                if slot == "Hips" {
                    c.stance_hips = p.to_array();
                }
            }
            None => {
                let c = &mut self.characters[i];
                c.joints.remove(slot);
                c.stance.remove(slot);
            }
        }
        self.touch();
    }

    pub fn hik_delete(&mut self, ch: &str) {
        self.hik_delete_control_rig(ch);
        self.characters.retain(|c| c.name != ch);
        for c in &mut self.characters {
            if c.source == ch {
                c.source.clear();
            }
        }
        self.touch();
    }

    pub fn hik_set_source(&mut self, ch: &str, source: &str) -> Result<(), String> {
        let i = self.hik_index(ch).ok_or_else(|| format!("No character {ch}"))?;
        if source == ch {
            return Err("A character can't drive itself".into());
        }
        if source == SOURCE_CONTROL_RIG && self.characters[i].rig.is_none() {
            return Err(format!("{ch} has no control rig"));
        }
        if !source.is_empty() && source != SOURCE_CONTROL_RIG {
            let src = self.hik_character(source).ok_or_else(|| format!("No character {source}"))?;
            if src.joints.get("Hips").is_none() {
                return Err(format!("{source} has no Hips mapped"));
            }
        }
        self.characters[i].source = source.into();
        let moved = self.eval_constraints();
        self.refresh_many(&moved);
        self.touch();
        Ok(())
    }

    // ---------------------------------------------------------------- skeleton

    /// Create a standard HumanIK skeleton `height` units tall in a T-pose, and its character.
    pub fn hik_create_skeleton(&mut self, name: &str, height: f64, spines: usize, fingers: bool) -> Result<String, String> {
        let name = self.hik_unique(name);
        let h = height.max(1.0);
        let mut ids: BTreeMap<String, Id> = BTreeMap::new();
        let mut add = |s: &mut Scene, slot: &str, parent: Option<&str>, p: [f64; 3]| {
            let pid = parent.and_then(|p| ids.get(p).cloned());
            let id = s.create_joint_at(DVec3::from_array(p) * h, pid.as_deref(), &format!("{name}_{slot}"));
            if let Some(n) = s.nodes.get_mut(&id) {
                n.passthrough.insert("rig".into(), serde_json::json!({ "radius": h / 90.0 }));
            }
            ids.insert(slot.into(), id);
        };
        add(self, "Hips", None, [0.0, 0.53, 0.0]);
        let spines = spines.clamp(1, 10);
        let (sp0, sp1) = (0.58, 0.76);
        let mut last = "Hips".to_string();
        for i in 0..spines {
            let slot = if i == 0 { "Spine".to_string() } else { format!("Spine{i}") };
            let y = sp0 + (sp1 - sp0) * i as f64 / spines.max(2).saturating_sub(1).max(1) as f64;
            add(self, &slot, Some(&last), [0.0, if spines == 1 { 0.62 } else { y }, 0.0]);
            last = slot;
        }
        let chest = last.clone();
        add(self, "Neck", Some(&chest), [0.0, 0.83, 0.0]);
        add(self, "Head", Some("Neck"), [0.0, 0.88, 0.0]);
        for (side, x) in [("Left", 1.0), ("Right", -1.0)] {
            let sl = |s: &str| format!("{side}{s}");
            add(self, &sl("Shoulder"), Some(&chest), [0.02 * x, 0.81, 0.0]);
            add(self, &sl("Arm"), Some(&sl("Shoulder")), [0.10 * x, 0.81, 0.0]);
            add(self, &sl("ForeArm"), Some(&sl("Arm")), [0.26 * x, 0.81, -0.005]);
            add(self, &sl("Hand"), Some(&sl("ForeArm")), [0.40 * x, 0.81, 0.0]);
            add(self, &sl("UpLeg"), Some("Hips"), [0.055 * x, 0.50, 0.0]);
            add(self, &sl("Leg"), Some(&sl("UpLeg")), [0.055 * x, 0.28, 0.008]);
            add(self, &sl("Foot"), Some(&sl("Leg")), [0.055 * x, 0.045, -0.01]);
            add(self, &sl("ToeBase"), Some(&sl("Foot")), [0.055 * x, 0.01, 0.07]);
            if fingers {
                // (finger, start offset from the hand, segment direction)
                let fs: [(&str, [f64; 3], [f64; 3]); 5] = [
                    ("Thumb", [0.015, -0.01, 0.02], [0.018, -0.004, 0.012]),
                    ("Index", [0.045, 0.0, 0.016], [0.022, 0.0, 0.0]),
                    ("Middle", [0.047, 0.0, 0.004], [0.024, 0.0, 0.0]),
                    ("Ring", [0.045, 0.0, -0.008], [0.022, 0.0, 0.0]),
                    ("Pinky", [0.04, -0.002, -0.019], [0.017, 0.0, 0.0]),
                ];
                for (f, o, d) in fs {
                    let mut parent = sl("Hand");
                    for k in 1..=3 {
                        let slot = sl(&format!("Hand{f}{k}"));
                        let p = [(0.40 + o[0] + d[0] * (k - 1) as f64) * x, 0.81 + o[1] + d[1] * (k - 1) as f64, o[2] + d[2] * (k - 1) as f64];
                        add(self, &slot, Some(&parent), p);
                        parent = slot;
                    }
                }
            }
        }
        let root = ids["Hips"].clone();
        self.orient_hierarchy(std::slice::from_ref(&root));
        self.characters.push(Character { name: name.clone(), joints: ids, ..Default::default() });
        let i = self.characters.len() - 1;
        self.hik_capture_stance(i);
        self.touch();
        Ok(name)
    }

    // ---------------------------------------------------------------- control rig

    /// Build a control rig: FK controls for the hips, spine, neck, head and shoulders, and IK
    /// effectors with pole vectors for the hands and feet.
    pub fn hik_create_control_rig(&mut self, ch: &str) -> Result<Id, String> {
        let i = self.hik_index(ch).ok_or_else(|| format!("No character {ch}"))?;
        let c = self.characters[i].clone();
        if c.rig.is_some() {
            return Err(format!("{ch} already has a control rig"));
        }
        let missing = c.missing();
        if !missing.is_empty() {
            return Err(format!("The definition is missing: {}", missing.join(", ")));
        }
        let j = |s: &str| c.joints[s].clone();
        let hips_h = self.world_pos(&j("Hips")).y.abs().max(1e-3);
        let mut rig = ControlRig::default();
        let ctrl = |s: &mut Scene, name: &str, r: f64, normal: DVec3, pos: DVec3, parent: Option<&str>| -> Id {
            let id = s.create_curve(crate::curves::nurbs_circle(r, 8, normal), &format!("{ch}_Ctrl_{name}"));
            if let Some(p) = parent {
                s.reparent(&id, Some(p), None);
            }
            s.put_world(&id, DMat4::from_translation(pos));
            id
        };
        let hp = self.world_pos(&j("Hips"));
        let reference = ctrl(self, "Reference", hips_h * 0.6, DVec3::Y, DVec3::new(hp.x, 0.0, hp.z), None);
        rig.root = reference.clone();
        rig.controls.insert("Reference".into(), reference.clone());
        let hips = ctrl(self, "Hips", hips_h * 0.35, DVec3::Y, hp, Some(&reference));
        rig.controls.insert("Hips".into(), hips.clone());
        rig.constraints.push(self.add_constraint("parent", vec![hips.clone()], &j("Hips"), true));
        // FK: each body slot under the control of its nearest mapped ancestor
        let fk: Vec<&str> = SLOTS.iter().map(|s| s.0).filter(|s| s.starts_with("Spine") || s.starts_with("Neck") || *s == "Head" || s.ends_with("Shoulder")).filter(|s| c.joints.contains_key(*s)).collect();
        for slot in fk {
            let jid = j(slot);
            let mut parent = hips.clone();
            let mut a = self.node(&jid).and_then(|n| n.parent.clone());
            while let Some(p) = a {
                if let Some((k, _)) = c.joints.iter().find(|(_, v)| **v == p) {
                    if let Some(pc) = rig.controls.get(k) {
                        parent = pc.clone();
                        break;
                    }
                }
                a = self.node(&p).and_then(|n| n.parent.clone());
            }
            let (r, normal) = if slot.ends_with("Shoulder") { (hips_h * 0.08, DVec3::X) } else if slot == "Head" { (hips_h * 0.18, DVec3::Y) } else { (hips_h * 0.26, DVec3::Y) };
            let id = ctrl(self, slot, r, normal, self.world_pos(&jid), Some(&parent));
            rig.controls.insert(slot.into(), id.clone());
            rig.constraints.push(self.add_constraint("orient", vec![id], &jid, true));
        }
        let chest = ["Spine9", "Spine8", "Spine7", "Spine6", "Spine5", "Spine4", "Spine3", "Spine2", "Spine1", "Spine"].iter().find_map(|s| rig.controls.get(*s)).cloned().unwrap_or(hips.clone());
        // IK limbs: hands follow the chest, feet stay in world space
        for side in ["Left", "Right"] {
            for (a, b, e, leg) in [("UpLeg", "Leg", "Foot", true), ("Arm", "ForeArm", "Hand", false)] {
                let (ja, jb, je) = (j(&format!("{side}{a}")), j(&format!("{side}{b}")), j(&format!("{side}{e}")));
                let ik = self.create_ik_handle(&ja, &je, "ikRPsolver")?;
                crate::commands::rename(self, &ik, &format!("{ch}_{side}{e}_ikHandle"));
                let pe = self.world_pos(&je);
                let parent = if leg { reference.clone() } else { chest.clone() };
                let eff = ctrl(self, &format!("{side}{e}Effector"), hips_h * if leg { 0.12 } else { 0.09 }, if leg { DVec3::Y } else { DVec3::X }, pe, Some(&parent));
                self.reparent(&ik, Some(&eff), None);
                self.put_world(&ik, DMat4::from_translation(pe));
                if let Some(n) = self.nodes.get_mut(&ik) {
                    n.visible = false;
                }
                // pole vector in front of the knee / behind the elbow
                let (pa, pb) = (self.world_pos(&ja), self.world_pos(&jb));
                let mid = (pa + pe) * 0.5;
                let mut bend = pb - mid;
                let ax = (pe - pa).normalize_or(DVec3::NEG_Y);
                bend -= ax * bend.dot(ax);
                let bend = if bend.length() > 1e-4 { bend.normalize() } else if leg { DVec3::Z } else { DVec3::NEG_Z };
                let pole = self.create_locator(&format!("{ch}_Ctrl_{side}{b}Pole"));
                self.reparent(&pole, Some(&parent), None);
                self.put_world(&pole, DMat4::from_translation(pb + bend * hips_h * 0.5));
                rig.constraints.push(self.add_constraint("poleVector", vec![pole.clone()], &ik, false));
                let eq = self.world_matrix(&eff).to_scale_rotation_translation().1;
                let jq = self.world_matrix(&je).to_scale_rotation_translation().1;
                rig.offsets.insert(format!("{side}{e}"), q4(eq.inverse() * jq));
                rig.controls.insert(format!("{side}{e}"), eff);
                rig.controls.insert(format!("{side}{b}Pole"), pole);
                rig.ik_handles.push(ik);
            }
        }
        let root = rig.root.clone();
        let c = &mut self.characters[i];
        c.rig = Some(rig);
        c.source = SOURCE_CONTROL_RIG.into();
        let moved = self.eval_constraints();
        self.refresh_many(&moved);
        self.touch();
        Ok(root)
    }

    pub fn hik_delete_control_rig(&mut self, ch: &str) {
        let Some(i) = self.hik_index(ch) else { return };
        let Some(rig) = self.characters[i].rig.take() else { return };
        if self.characters[i].source == SOURCE_CONTROL_RIG {
            self.characters[i].source.clear();
        }
        self.constraints.retain(|c| !rig.constraints.contains(&c.name));
        if self.nodes.contains_key(&rig.root) {
            self.delete_node(&rig.root);
        }
        self.touch();
    }

    // ---------------------------------------------------------------- evaluation

    /// Live HumanIK step, after constraints and IK: effector orientations and retargeting.
    pub fn eval_hik(&mut self) -> Vec<Id> {
        let mut moved = vec![];
        for i in 0..self.characters.len() {
            let c = self.characters[i].clone();
            if c.source == SOURCE_CONTROL_RIG {
                if let Some(rig) = &c.rig {
                    for (slot, off) in &rig.offsets {
                        let (Some(j), Some(ctl)) = (c.joints.get(slot), rig.controls.get(slot)) else { continue };
                        if !self.nodes.contains_key(j) || !self.nodes.contains_key(ctl) {
                            continue;
                        }
                        let q = self.world_matrix(ctl).to_scale_rotation_translation().1 * quat(off);
                        self.set_world_rotation(j, q);
                        moved.push(j.clone());
                    }
                }
            } else if !c.source.is_empty() {
                if let Some(src) = self.hik_character(&c.source).cloned() {
                    moved.extend(self.retarget(&src, &c));
                }
            }
        }
        moved
    }

    fn set_world_rotation(&mut self, id: &str, q: DQuat) {
        let (s, _, t) = self.world_matrix(id).to_scale_rotation_translation();
        self.put_world(id, DMat4::from_scale_rotation_translation(s, q, t));
    }

    /// Pose `tgt` from `src`: world rotations relative to each stance, hips motion scaled by height.
    fn retarget(&mut self, src: &Character, tgt: &Character) -> Vec<Id> {
        let mut slots: Vec<(&String, &Id)> = tgt.joints.iter().filter(|(k, j)| *k != "Reference" && self.nodes.contains_key(*j) && tgt.stance.contains_key(*k)).collect();
        slots.sort_by_key(|(_, j)| self.joint_depth(j));
        let mut moved = vec![];
        for (slot, j) in slots {
            let ts = quat(&tgt.stance[slot]);
            let q = match (src.joints.get(slot).filter(|s| self.nodes.contains_key(*s)), src.stance.get(slot)) {
                (Some(sj), Some(ss)) => {
                    let now = self.world_matrix(sj).to_scale_rotation_translation().1;
                    now * quat(ss).inverse() * ts
                }
                // not in the source: keep its stance relative to its parent slot
                _ => {
                    let parent = self.node(j).and_then(|n| n.parent.clone());
                    let Some((pslot, pj)) = parent.and_then(|p| tgt.joints.iter().find(|(_, v)| **v == p).map(|(k, v)| (k.clone(), v.clone()))) else { continue };
                    let Some(ps) = tgt.stance.get(&pslot) else { continue };
                    self.world_matrix(&pj).to_scale_rotation_translation().1 * quat(ps).inverse() * ts
                }
            };
            self.set_world_rotation(j, q);
            if slot == "Hips" {
                if let Some(sh) = src.joints.get("Hips").filter(|s| self.nodes.contains_key(*s)) {
                    let (s0, t0) = (DVec3::from_array(src.stance_hips), DVec3::from_array(tgt.stance_hips));
                    let k = if s0.y.abs() > 1e-6 { t0.y / s0.y } else { 1.0 };
                    let p = t0 + (self.world_pos(sh) - s0) * k;
                    let (sc, q, _) = self.world_matrix(j).to_scale_rotation_translation();
                    self.put_world(j, DMat4::from_scale_rotation_translation(sc, q, p));
                }
            }
            moved.push(j.clone());
        }
        moved
    }

    /// Bake what drives `ch` (another character or its control rig) into rotate keys on its
    /// joints (and translate keys on the hips) for frames start..=end; the character is then
    /// driven by its keys alone (a baked control rig is removed). Returns the number of keys.
    pub fn hik_bake(&mut self, ch: &str, start: f64, end: f64) -> Result<usize, String> {
        let i = self.hik_index(ch).ok_or_else(|| format!("No character {ch}"))?;
        let c = self.characters[i].clone();
        if c.source.is_empty() {
            return Err(format!("{ch} has no source to bake"));
        }
        let now = self.time.current;
        let joints: Vec<(String, Id)> = c.joints.iter().filter(|(k, j)| *k != "Reference" && self.nodes.contains_key(*j)).map(|(k, j)| (k.clone(), j.clone())).collect();
        let mut frames: Vec<(f64, Vec<(Id, [f64; 3], [f64; 3])>)> = vec![];
        let mut f = start;
        while f <= end + 1e-9 {
            self.set_time(f);
            frames.push((f, joints.iter().map(|(_, j)| (j.clone(), self.nodes[j].xf.r, self.nodes[j].xf.t)).collect()));
            f += 1.0;
        }
        // stop the live drive, then key
        if c.source == SOURCE_CONTROL_RIG {
            self.hik_delete_control_rig(ch);
        }
        if let Some(i) = self.hik_index(ch) {
            self.characters[i].source.clear();
        }
        let hips = c.joints.get("Hips").cloned();
        let mut n = 0;
        for (f, vals) in frames {
            for (j, r, t) in vals {
                for (k, a) in ["rotateX", "rotateY", "rotateZ"].iter().enumerate() {
                    self.set_key(&j, a, Some(f), Some(r[k]));
                    n += 1;
                }
                if Some(&j) == hips.as_ref() {
                    for (k, a) in ["translateX", "translateY", "translateZ"].iter().enumerate() {
                        self.set_key(&j, a, Some(f), Some(t[k]));
                        n += 1;
                    }
                }
            }
        }
        self.set_time(now);
        self.touch();
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_slots() {
        let cases = [
            ("mixamorig:LeftUpLeg", Some("LeftUpLeg")),
            ("mixamorig:RightHandIndex2", Some("RightHandIndex2")),
            ("mixamorig:LeftHandIndex4", None),
            ("thigh_l", Some("LeftUpLeg")),
            ("calf_r", Some("RightLeg")),
            ("upperarm_l", Some("LeftArm")),
            ("lowerarm_r", Some("RightForeArm")),
            ("clavicle_l", Some("LeftShoulder")),
            ("index_01_l", Some("LeftHandIndex1")),
            ("Arm_L", Some("LeftArm")),
            ("DEF-forearm_R", Some("RightForeArm")),
            ("lShldr", Some("LeftArm")),
            ("rCollar", Some("RightShoulder")),
            ("Bip01_L_Thigh", Some("LeftUpLeg")),
            ("pelvis", Some("Hips")),
            ("Character1_Head", Some("Head")),
            ("HeadTop_End", None),
            ("upperarm_twist_01_l", None),
        ];
        for (n, want) in cases {
            assert_eq!(classify(n).as_deref(), want, "{n}");
        }
    }

    #[test]
    fn skeleton_is_fully_defined() {
        let mut s = Scene::new();
        let ch = s.hik_create_skeleton("Character1", 170.0, 3, true).unwrap();
        let c = s.hik_character(&ch).unwrap();
        assert!(c.missing().is_empty());
        assert_eq!(c.joints.len(), 1 + 3 + 2 + 2 * (4 + 4 + 15));
        // auto-mapping the same skeleton finds every slot again
        let map = s.hik_auto_map(&c.joints["Hips"]);
        for (k, v) in &c.joints {
            assert_eq!(map.get(k), Some(v), "{k}");
        }
    }

    #[test]
    fn unreal_style_skeleton_is_characterized() {
        let mut s = Scene::new();
        let j = |s: &mut Scene, n: &str, p: Option<&str>, x: f64, y: f64| {
            let parent = p.and_then(|p| s.find(p));
            s.create_joint_at(DVec3::new(x, y, 0.0), parent.as_deref(), n);
        };
        j(&mut s, "root", None, 0.0, 0.0);
        j(&mut s, "pelvis", Some("root"), 0.0, 90.0);
        j(&mut s, "spine_01", Some("pelvis"), 0.0, 100.0);
        j(&mut s, "spine_02", Some("spine_01"), 0.0, 115.0);
        j(&mut s, "spine_03", Some("spine_02"), 0.0, 130.0);
        j(&mut s, "neck_01", Some("spine_03"), 0.0, 145.0);
        j(&mut s, "head", Some("neck_01"), 0.0, 155.0);
        for (sd, x) in [("l", 1.0), ("r", -1.0)] {
            j(&mut s, &format!("clavicle_{sd}"), Some("spine_03"), 5.0 * x, 140.0);
            j(&mut s, &format!("upperarm_{sd}"), Some(&format!("clavicle_{sd}")), 15.0 * x, 140.0);
            j(&mut s, &format!("lowerarm_{sd}"), Some(&format!("upperarm_{sd}")), 40.0 * x, 140.0);
            j(&mut s, &format!("hand_{sd}"), Some(&format!("lowerarm_{sd}")), 65.0 * x, 140.0);
            j(&mut s, &format!("thigh_{sd}"), Some("pelvis"), 10.0 * x, 88.0);
            j(&mut s, &format!("calf_{sd}"), Some(&format!("thigh_{sd}")), 10.0 * x, 48.0);
            j(&mut s, &format!("foot_{sd}"), Some(&format!("calf_{sd}")), 10.0 * x, 8.0);
            j(&mut s, &format!("ball_{sd}"), Some(&format!("foot_{sd}")), 10.0 * x, 2.0);
        }
        let root = s.find("root").unwrap();
        let ch = s.hik_define(&root, None).unwrap();
        let c = s.hik_character(&ch).unwrap();
        assert!(c.missing().is_empty(), "{:?}", c.missing());
        let name = |slot: &str| s.name_of(&c.joints[slot]);
        assert_eq!((name("Hips"), name("Spine"), name("Spine2"), name("Neck"), name("Reference")), ("pelvis".into(), "spine_01".into(), "spine_03".into(), "neck_01".into(), "root".into()));
        assert_eq!((name("LeftShoulder"), name("RightToeBase")), ("clavicle_l".into(), "ball_r".into()));
    }

    #[test]
    fn retargets_rotations_and_scaled_hips() {
        let mut s = Scene::new();
        let a = s.hik_create_skeleton("Src", 100.0, 2, false).unwrap();
        let b = s.hik_create_skeleton("Tgt", 200.0, 4, false).unwrap();
        s.hik_set_source(&b, &a).unwrap();
        let (sa, sb) = (s.hik_character(&a).unwrap().clone(), s.hik_character(&b).unwrap().clone());
        // the source raises its left arm and its hips drop 10
        let arm = sa.joints["LeftArm"].clone();
        let q0 = s.world_matrix(&arm).to_scale_rotation_translation().1;
        let up = DQuat::from_rotation_z(0.8) * q0;
        let (sc, _, t) = s.world_matrix(&arm).to_scale_rotation_translation();
        s.put_world(&arm, DMat4::from_scale_rotation_translation(sc, up, t));
        let hips = sa.joints["Hips"].clone();
        s.nodes.get_mut(&hips).unwrap().xf.t[1] -= 10.0;
        let moved = s.eval_constraints();
        s.refresh_many(&moved);
        let tq = s.world_matrix(&sb.joints["LeftArm"]).to_scale_rotation_translation().1;
        let want = DQuat::from_rotation_z(0.8) * DQuat::from_array(sb.stance["LeftArm"]);
        assert!(tq.angle_between(want) < 1e-6, "{}", tq.angle_between(want));
        let hy = s.world_pos(&sb.joints["Hips"]).y;
        assert!((hy - (sb.stance_hips[1] - 20.0)).abs() < 1e-6, "{hy}");
        // and it bakes
        let n = s.hik_bake(&b, 1.0, 3.0).unwrap();
        assert!(n > 0);
        assert!(s.hik_character(&b).unwrap().source.is_empty());
        assert!(s.anim.curves.values().any(|c| c.node == sb.joints["LeftArm"] && c.keys.len() == 3));
    }

    #[test]
    fn control_rig_drives_the_skeleton() {
        let mut s = Scene::new();
        let ch = s.hik_create_skeleton("Character1", 170.0, 3, false).unwrap();
        let c = s.hik_character(&ch).unwrap().clone();
        let foot = c.joints["LeftFoot"].clone();
        let rfoot = c.joints["RightFoot"].clone();
        let p0 = s.world_pos(&foot);
        let r0 = s.world_pos(&rfoot);
        s.hik_create_control_rig(&ch).unwrap();
        let rig = s.hik_character(&ch).unwrap().rig.clone().unwrap();
        // creating the rig doesn't move anything
        assert!(s.world_pos(&foot).distance(p0) < 1e-6);
        // lift the left foot effector
        let eff = rig.controls["LeftFoot"].clone();
        s.nodes.get_mut(&eff).unwrap().xf.t[1] += 20.0;
        let moved = s.eval_constraints();
        s.refresh_many(&moved);
        assert!(s.world_pos(&foot).distance(p0 + DVec3::new(0.0, 20.0, 0.0)) < 1e-3, "{}", s.world_pos(&foot));
        // drop the hips: the right foot stays planted
        let hips = rig.controls["Hips"].clone();
        s.nodes.get_mut(&hips).unwrap().xf.t[1] -= 10.0;
        let moved = s.eval_constraints();
        s.refresh_many(&moved);
        assert!(s.world_pos(&rfoot).distance(r0) < 1e-3, "{}", s.world_pos(&rfoot));
        assert!((s.world_pos(&c.joints["Hips"]).y - (c.stance_hips[1] - 10.0)).abs() < 1e-6);
        s.hik_delete_control_rig(&ch);
        assert!(!s.nodes.contains_key(&eff));
        assert!(s.hik_character(&ch).unwrap().rig.is_none());
    }

    #[test]
    fn characters_survive_save_and_undo() {
        let mut s = Scene::new();
        let ch = s.hik_create_skeleton("Character1", 170.0, 3, false).unwrap();
        s.hik_create_control_rig(&ch).unwrap();
        let data = s.serialize();
        let mut t = Scene::new();
        t.load(&data).unwrap();
        assert_eq!(t.characters, s.characters);
    }

    #[test]
    fn fbx_skeleton_characterizes_and_retargets() {
        let mut s = Scene::new();
        s.hik_create_skeleton("Mocap", 180.0, 3, true).unwrap();
        let bytes = crate::fbx::export_fbx(&s, false);
        let mut t = Scene::new();
        crate::fbx::import_fbx(&mut t, &bytes, None, "mocap").unwrap();
        let root = t.find("Mocap_Hips").unwrap();
        let src = t.hik_define(&root, Some("Mocap")).unwrap();
        assert!(t.hik_character(&src).unwrap().missing().is_empty());
        assert_eq!(t.hik_character(&src).unwrap().joints.len(), 1 + 3 + 2 + 2 * (4 + 4 + 15));
        let tgt = t.hik_create_skeleton("Hero", 120.0, 2, false).unwrap();
        t.hik_set_source(&tgt, &src).unwrap();
        let fore = t.find("Mocap_LeftForeArm").unwrap();
        t.nodes.get_mut(&fore).unwrap().xf.r[1] += 60.0;
        let moved = t.eval_constraints();
        t.refresh_many(&moved);
        let tc = t.hik_character(&tgt).unwrap().clone();
        let sc = t.hik_character(&src).unwrap().clone();
        let delta = |c: &Character, id: &str, slot: &str| t.world_matrix(id).to_scale_rotation_translation().1 * quat(&c.stance[slot]).inverse();
        let (a, b) = (delta(&sc, &fore, "LeftForeArm"), delta(&tc, &tc.joints["LeftForeArm"], "LeftForeArm"));
        assert!(a.angle_between(b) < 1e-6 && a.angle_between(DQuat::IDENTITY) > 0.5);
    }
}
