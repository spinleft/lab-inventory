/*
 * @Author: spinleft spinleftgit@gmail.com
 * @Date: 2025-10-19 17:29:05
 * @LastEditors: spinleft spinleftgit@gmail.com
 * @LastEditTime: 2025-10-19 23:30:00
 * @FilePath: \lab-inventory\frontend\src\App.jsx
 * @Description: 
 * 
 * Copyright (c) 2025 by ${git_name_email}, All Rights Reserved. 
 */
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import './App.css';

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/dashboard" element={<Dashboard />} />
        <Route path="/" element={<Navigate to="/login" replace />} />
      </Routes>
    </Router>
  );
}

export default App;
