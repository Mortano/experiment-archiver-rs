--
-- PostgreSQL database dump
--

-- Dumped from database version 12.5 (Debian 12.5-1.pgdg100+1)
-- Dumped by pg_dump version 13.2

-- Started on 2023-10-11 15:52:40 CEST

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- TOC entry 204 (class 1259 OID 2210510)
-- Name: experiment_instances; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiment_instances (
    id character varying(16) NOT NULL,
    name text NOT NULL,
    version_id character varying(16) NOT NULL
);


ALTER TABLE public.experiment_instances OWNER TO postgres;

--
-- TOC entry 206 (class 1259 OID 2210536)
-- Name: experiment_variables; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiment_variables (
    var_name text NOT NULL,
    ex_name text NOT NULL,
    ex_version_id character varying(16) NOT NULL,
    kind text NOT NULL,
    UNIQUE (var_name, ex_version_id)
);


ALTER TABLE public.experiment_variables OWNER TO postgres;

--
-- TOC entry 203 (class 1259 OID 2210497)
-- Name: experiment_versions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiment_versions (
    id character varying(16) NOT NULL,
    version text NOT NULL,
    name text NOT NULL,
    date timestamp with time zone NOT NULL,
    description text,
    researchers text
);


ALTER TABLE public.experiment_versions OWNER TO postgres;

--
-- TOC entry 202 (class 1259 OID 2210489)
-- Name: experiments; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiments (
    name text NOT NULL
);


ALTER TABLE public.experiments OWNER TO postgres;

--
-- TOC entry 207 (class 1259 OID 2210557)
-- Name: in_values; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.in_values (
    ex_instance_id character varying(16) NOT NULL,
    var_name text NOT NULL,
    value text NOT NULL
);


ALTER TABLE public.in_values OWNER TO postgres;

--
-- TOC entry 209 (class 1259 OID 2210583)
-- Name: measurements; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.measurements (
    run_id character varying(16) NOT NULL,
    var_name text NOT NULL,
    value text NOT NULL
);


ALTER TABLE public.measurements OWNER TO postgres;

--
-- TOC entry 208 (class 1259 OID 2210573)
-- Name: runs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.runs (
    id character varying(16) NOT NULL,
    ex_instance_id character varying(16) NOT NULL,
    date timestamp with time zone NOT NULL
);


ALTER TABLE public.runs OWNER TO postgres;

--
-- TOC entry 205 (class 1259 OID 2210528)
-- Name: variables; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.variables (
    name text NOT NULL,
    description text,
    type text NOT NULL
);


ALTER TABLE public.variables OWNER TO postgres;

--
-- TOC entry 2815 (class 2606 OID 2210517)
-- Name: experiment_instances experiment_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_instances
    ADD CONSTRAINT experiment_instances_pkey PRIMARY KEY (id);


--
-- TOC entry 2813 (class 2606 OID 2210504)
-- Name: experiment_versions experiment_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_versions
    ADD CONSTRAINT experiment_versions_pkey PRIMARY KEY (id);


--
-- TOC entry 2811 (class 2606 OID 2210496)
-- Name: experiments experiments_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiments
    ADD CONSTRAINT experiments_pkey PRIMARY KEY (name);


--
-- TOC entry 2819 (class 2606 OID 2210577)
-- Name: runs runs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.runs
    ADD CONSTRAINT runs_pkey PRIMARY KEY (id);


--
-- TOC entry 2817 (class 2606 OID 2210535)
-- Name: variables variables_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.variables
    ADD CONSTRAINT variables_pkey PRIMARY KEY (name);


--
-- TOC entry 2826 (class 2606 OID 2210563)
-- Name: in_values experiment_instance_id; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.in_values
    ADD CONSTRAINT experiment_instance_id FOREIGN KEY (ex_instance_id) REFERENCES public.experiment_instances(id);


--
-- TOC entry 2828 (class 2606 OID 2210578)
-- Name: runs experiment_instance_id; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.runs
    ADD CONSTRAINT experiment_instance_id FOREIGN KEY (ex_instance_id) REFERENCES public.experiment_instances(id);


--
-- TOC entry 2824 (class 2606 OID 2210547)
-- Name: experiment_variables experiment_name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_variables
    ADD CONSTRAINT experiment_name FOREIGN KEY (ex_name) REFERENCES public.experiments(name);


--
-- TOC entry 2825 (class 2606 OID 2210552)
-- Name: experiment_variables experiment_version; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_variables
    ADD CONSTRAINT experiment_version FOREIGN KEY (ex_version_id) REFERENCES public.experiment_versions(id);


--
-- TOC entry 2820 (class 2606 OID 2210505)
-- Name: experiment_versions experiments.name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_versions
    ADD CONSTRAINT "experiments.name" FOREIGN KEY (name) REFERENCES public.experiments(name);


--
-- TOC entry 2821 (class 2606 OID 2210518)
-- Name: experiment_instances name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_instances
    ADD CONSTRAINT name FOREIGN KEY (name) REFERENCES public.experiments(name);


--
-- TOC entry 2829 (class 2606 OID 2210589)
-- Name: measurements run_id; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.measurements
    ADD CONSTRAINT run_id FOREIGN KEY (run_id) REFERENCES public.runs(id);


--
-- TOC entry 2823 (class 2606 OID 2210542)
-- Name: experiment_variables variable_name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_variables
    ADD CONSTRAINT variable_name FOREIGN KEY (var_name) REFERENCES public.variables(name);


--
-- TOC entry 2827 (class 2606 OID 2210568)
-- Name: in_values variable_name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.in_values
    ADD CONSTRAINT variable_name FOREIGN KEY (var_name) REFERENCES public.variables(name);


--
-- TOC entry 2830 (class 2606 OID 2210594)
-- Name: measurements variable_name; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.measurements
    ADD CONSTRAINT variable_name FOREIGN KEY (var_name) REFERENCES public.variables(name);


--
-- TOC entry 2822 (class 2606 OID 2210523)
-- Name: experiment_instances version; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_instances
    ADD CONSTRAINT version_id FOREIGN KEY (version_id) REFERENCES public.experiment_versions(id);


-- Completed on 2023-10-11 15:52:40 CEST

--
-- PostgreSQL database dump complete
--

