//!  # Math
//!
//!  Provides [`Matrix`] interface for simplifying the 3D matrix maths.
//!
//!  Every matrix is represented as its transpose in memory and this module.
//!  See display impl for actual representation.
//!
//!  ## Reference
//!  https://webgpufundamentals.org/webgpu/lessons/webgpu-orthographic-projection.html
// TODO make sure positive always rotates clockwise

use std::{
    fmt::Display,
    ops::{Index, Mul},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Matrix([[f32; 4]; 4]);

impl Matrix {
    pub fn new(stock_size: [f32; 3]) -> Self {
        Self::identity().translate([
            -stock_size[0] / 2.0,
            -stock_size[1] / 2.0,
            -stock_size[2] / 2.0,
        ])
    }

    const fn identity() -> Self {
        Self([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    pub fn translate(mut self, delta: [f32; 3]) -> Self {
        self.0[3][0] += delta[0];
        self.0[3][1] += delta[1];
        self.0[3][2] += delta[2];
        self
    }

    // also scales the translations
    pub fn scale(mut self, scales: [f32; 3]) -> Self {
        self.0[0][0] *= scales[0];
        self.0[1][0] *= scales[0];
        self.0[2][0] *= scales[0];
        self.0[3][0] *= scales[0];

        self.0[0][1] *= scales[1];
        self.0[1][1] *= scales[1];
        self.0[2][1] *= scales[1];
        self.0[3][1] *= scales[1];

        self.0[0][2] *= scales[2];
        self.0[1][2] *= scales[2];
        self.0[2][2] *= scales[2];
        self.0[3][2] *= scales[2];

        self
    }

    // rotation order: z, y, x
    pub fn rotate(self, rotations: [f32; 3]) -> Self {
        Matrix::rotation_x(rotations[0])
            * (Matrix::rotation_y(rotations[1]) * (Matrix::rotation_z(rotations[2]) * self))
    }

    // rads
    fn rotation_x(angle: f32) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();

        Self([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, cos, -sin, 0.0],
            [0.0, sin, cos, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn rotation_y(angle: f32) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();

        Self([
            [cos, 0.0, -sin, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [sin, 0.0, cos, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn rotation_z(angle: f32) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();

        Self([
            [cos, sin, 0.0, 0.0],
            [-sin, cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
}

impl Index<usize> for Matrix {
    type Output = [f32; 4];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Mul for Matrix {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self([
            [
                self[0][0] * rhs[0][0]
                    + self[1][0] * rhs[0][1]
                    + self[2][0] * rhs[0][2]
                    + self[3][0] * rhs[0][3],
                self[0][1] * rhs[0][0]
                    + self[1][1] * rhs[0][1]
                    + self[2][1] * rhs[0][2]
                    + self[3][1] * rhs[0][3],
                self[0][2] * rhs[0][0]
                    + self[1][2] * rhs[0][1]
                    + self[2][2] * rhs[0][2]
                    + self[3][2] * rhs[0][3],
                self[0][3] * rhs[0][0]
                    + self[1][3] * rhs[0][1]
                    + self[2][3] * rhs[0][2]
                    + self[3][3] * rhs[0][3],
            ],
            [
                self[0][0] * rhs[1][0]
                    + self[1][0] * rhs[1][1]
                    + self[2][0] * rhs[1][2]
                    + self[3][0] * rhs[1][3],
                self[0][1] * rhs[1][0]
                    + self[1][1] * rhs[1][1]
                    + self[2][1] * rhs[1][2]
                    + self[3][1] * rhs[1][3],
                self[0][2] * rhs[1][0]
                    + self[1][2] * rhs[1][1]
                    + self[2][2] * rhs[1][2]
                    + self[3][2] * rhs[1][3],
                self[0][3] * rhs[1][0]
                    + self[1][3] * rhs[1][1]
                    + self[2][3] * rhs[1][2]
                    + self[3][3] * rhs[1][3],
            ],
            [
                self[0][0] * rhs[2][0]
                    + self[1][0] * rhs[2][1]
                    + self[2][0] * rhs[2][2]
                    + self[3][0] * rhs[2][3],
                self[0][1] * rhs[2][0]
                    + self[1][1] * rhs[2][1]
                    + self[2][1] * rhs[2][2]
                    + self[3][1] * rhs[2][3],
                self[0][2] * rhs[2][0]
                    + self[1][2] * rhs[2][1]
                    + self[2][2] * rhs[2][2]
                    + self[3][2] * rhs[2][3],
                self[0][3] * rhs[2][0]
                    + self[1][3] * rhs[2][1]
                    + self[2][3] * rhs[2][2]
                    + self[3][3] * rhs[2][3],
            ],
            [
                self[0][0] * rhs[3][0]
                    + self[1][0] * rhs[3][1]
                    + self[2][0] * rhs[3][2]
                    + self[3][0] * rhs[3][3],
                self[0][1] * rhs[3][0]
                    + self[1][1] * rhs[3][1]
                    + self[2][1] * rhs[3][2]
                    + self[3][1] * rhs[3][3],
                self[0][2] * rhs[3][0]
                    + self[1][2] * rhs[3][1]
                    + self[2][2] * rhs[3][2]
                    + self[3][2] * rhs[3][3],
                self[0][3] * rhs[3][0]
                    + self[1][3] * rhs[3][1]
                    + self[2][3] * rhs[3][2]
                    + self[3][3] * rhs[3][3],
            ],
        ])
    }
}

impl Display for Matrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mat = self.0;

        writeln!(
            f,
            "
            {:^8.2} {:^8.2} {:^8.2} {:^8.2}
            {:^8.2} {:^8.2} {:^8.2} {:^8.2}
            {:^8.2} {:^8.2} {:^8.2} {:^8.2}
            {:^8.2} {:^8.2} {:^8.2} {:^8.2}
            ",
            mat[0][0],
            mat[1][0],
            mat[2][0],
            mat[3][0],
            mat[0][1],
            mat[1][1],
            mat[2][1],
            mat[3][1],
            mat[0][2],
            mat[1][2],
            mat[2][2],
            mat[3][2],
            mat[0][3],
            mat[1][3],
            mat[2][3],
            mat[3][3],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    fn print() {
        let mat = Matrix::identity();

        println!("{mat}");

        let mat = Matrix([
            [0.0, 0.1, 0.2, 0.3],
            [1.0, 1.1, 1.2, 1.3],
            [2.0, 2.1, 2.2, 2.3],
            [3.0, 3.1, 3.2, 3.3],
        ]);

        println!("{mat}");
    }

    #[test]
    fn multiply() {
        let a = Matrix([
            [7.0, 5.0, -6.0, 4.0],
            [3.0, 2.0, 9.0, -4.0],
            [-4.0, 2.0, -3.0, 6.0],
            [-9.0, 5.0, -8.0, -8.0],
        ]);

        let b = Matrix([
            [5.0, -7.0, 3.0, 2.0],
            [3.0, -8.0, -9.0, 2.0],
            [-6.0, -8.0, 3.0, 6.0],
            [-5.0, 0.0, -2.0, 2.0],
        ]);

        let mut ab = a * b;

        println!("A: {a}");
        println!("B: {b}");
        println!("AB: {ab}");

        let oracle = Matrix([
            [-16.0, 27.0, -118.0, 50.0],
            [15.0, -9.0, -79.0, -26.0],
            [-132.0, -10.0, -93.0, -22.0],
            [-45.0, -19.0, 20.0, -48.0],
        ]);

        assert_eq!(ab, oracle);

        ab = ab * Matrix::identity();
        assert_eq!(ab, oracle);
    }
}
