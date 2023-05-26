--
-- PostgreSQL database dump
--

-- Dumped from database version 14.7 (Homebrew)
-- Dumped by pg_dump version 14.7 (Homebrew)

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
-- Name: experiment_variables; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiment_variables (
    experiment_id character varying(16) NOT NULL,
    variable_id character varying(16) NOT NULL
);


ALTER TABLE public.experiment_variables OWNER TO postgres;

--
-- Name: experiment_runs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiment_runs (
    runnumber integer NOT NULL,
    experimentid character varying(16) NOT NULL,
    id character varying(16) NOT NULL,
    "timestamp" timestamp without time zone
);


ALTER TABLE public.experiment_runs OWNER TO postgres;

--
-- Name: experiments; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.experiments (
    id character varying(16) NOT NULL,
    researcher character varying(80),
    name text,
    description text
);


ALTER TABLE public.experiments OWNER TO postgres;

--
-- Name: measurements; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.measurements (
    experimentid character varying(16),
    variableid character varying(16),
    runid character varying(16),
    value text,
    "timestamp" timestamp without time zone
);


ALTER TABLE public.measurements OWNER TO postgres;

--
-- Name: variables; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.variables (
    id character varying(16) NOT NULL,
    name text,
    description text,
    unit text
);


ALTER TABLE public.variables OWNER TO postgres;

--
-- Name: experiment_runs experiment_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_runs
    ADD CONSTRAINT experiment_runs_pkey PRIMARY KEY (id);


--
-- Name: experiments experiments_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiments
    ADD CONSTRAINT experiments_name_key UNIQUE (name);


--
-- Name: experiments experiments_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiments
    ADD CONSTRAINT experiments_pkey PRIMARY KEY (id);


--
-- Name: variables variables_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.variables
    ADD CONSTRAINT variables_name_key UNIQUE (name);


--
-- Name: variables variables_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.variables
    ADD CONSTRAINT variables_pkey PRIMARY KEY (id);


--
-- Name: experiment_variables experiment_variables_experiment_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_variables
    ADD CONSTRAINT experiment_variables_experiment_id_fkey FOREIGN KEY (experiment_id) REFERENCES public.experiments(id);


--
-- Name: experiment_variables experiment_variables_variable_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_variables
    ADD CONSTRAINT experiment_variables_variable_id_fkey FOREIGN KEY (variable_id) REFERENCES public.variables(id);


--
-- Name: experiment_runs experiment_runs_experimentid_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.experiment_runs
    ADD CONSTRAINT experiment_runs_experimentid_fkey FOREIGN KEY (experimentid) REFERENCES public.experiments(id);


--
-- Name: measurements measurements_experimentid_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.measurements
    ADD CONSTRAINT measurements_experimentid_fkey FOREIGN KEY (experimentid) REFERENCES public.experiments(id);


--
-- Name: measurements measurements_runid_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.measurements
    ADD CONSTRAINT measurements_runid_fkey FOREIGN KEY (runid) REFERENCES public.experiment_runs(id);


--
-- Name: measurements measurements_variableid_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.measurements
    ADD CONSTRAINT measurements_variableid_fkey FOREIGN KEY (variableid) REFERENCES public.variables(id);


--
-- PostgreSQL database dump complete
--

