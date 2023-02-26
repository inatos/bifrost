use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub handle: usize
}

#[derive(Component)]
pub struct Paddle;

#[derive(Component)]
pub struct Ball;

#[derive(Component, Default, Reflect, Deref, DerefMut)] 
pub struct Velocity(pub Vec2);

#[derive(Component)]
pub struct Collider;

#[derive(Component, Default, Reflect)]
pub struct CollisionEvent;

#[derive(Component)]
pub struct Brick;