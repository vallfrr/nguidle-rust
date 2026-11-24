#![allow(non_snake_case)]
//! Unity value types (copied by value, no engine).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Color { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Vector2 { pub x: f32, pub y: f32 }
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Vector3 { pub x: f32, pub y: f32, pub z: f32 }
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Vector4 { pub x: f32, pub y: f32, pub z: f32, pub w: f32 }
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Quaternion { pub x: f32, pub y: f32, pub z: f32, pub w: f32 }

pub fn UnityEngine_Color___ctor_3(r: f32, g: f32, b: f32) -> Color { Color { r, g, b, a: 1.0 } }
pub fn UnityEngine_Color___ctor_4(r: f32, g: f32, b: f32, a: f32) -> Color { Color { r, g, b, a } }
pub fn UnityEngine_Vector2___ctor_2(x: f32, y: f32) -> Vector2 { Vector2 { x, y } }
pub fn UnityEngine_Vector3___ctor_2(x: f32, y: f32) -> Vector3 { Vector3 { x, y, z: 0.0 } }
pub fn UnityEngine_Vector3___ctor_3(x: f32, y: f32, z: f32) -> Vector3 { Vector3 { x, y, z } }
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Color32 { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
pub fn UnityEngine_Color32___ctor_4(r: i32, g: i32, b: i32, a: i32) -> Color32 { Color32 { r: r as u8, g: g as u8, b: b as u8, a: a as u8 } }
