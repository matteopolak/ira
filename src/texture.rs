use std::{borrow::Cow, path::Path};

use anyhow::{anyhow, Ok, Result};
use glam::{Vec2, Vec3};
use image::GenericImageView;
use wgpu::util::DeviceExt;

use crate::Vertex;
